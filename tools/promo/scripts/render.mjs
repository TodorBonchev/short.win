#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { mkdir, open, readFile, rename, stat, unlink, writeFile } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const projectDir = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const outputDir = join(projectDir, 'out');
const entryPoint = 'src/index.ts';
const browser = '/usr/bin/chromium';
const durationSeconds = 56;
const frameCount = 1680;
const frameRate = 30;
const version = 'v1.0.3';
const variants = {
    landscape: { composition: 'PromoLandscape', width: 1920, height: 1080 },
    phone: { composition: 'PromoPhone', width: 1080, height: 1920 },
};

function usage() {
    console.log(`Usage: pnpm render [--variant landscape|phone|all] [--verify-only] [--preview]

Renders both 56-second compositions sequentially by default. Final files and
export-manifest.json go to out/. --preview renders half-size CRF 23 files to
out/draft/ with a separate manifest. --verify-only validates existing files
and refreshes the corresponding manifest without rendering or remuxing.`);
}

function parseArgs(args) {
    const options = { variant: 'all', verifyOnly: false, preview: false };
    for (let i = 0; i < args.length; i++) {
        const arg = args[i];
        if (arg === '--help' || arg === '-h') {
            usage();
            process.exit(0);
        }
        if (arg === '--variant') {
            options.variant = args[++i];
        } else if (arg === '--verify-only') {
            options.verifyOnly = true;
        } else if (arg === '--preview') {
            options.preview = true;
        } else {
            throw new Error(`Unknown argument: ${arg}`);
        }
    }
    if (!['all', ...Object.keys(variants)].includes(options.variant)) {
        throw new Error('--variant must be landscape, phone, or all');
    }
    return options;
}

// Drain child output continuously without retaining a full render's logs.
function run(command, args, { maxOutput = 32_768 } = {}) {
    return new Promise((resolveRun, reject) => {
        const child = spawn(command, args, {
            cwd: projectDir,
            stdio: ['ignore', 'pipe', 'pipe'],
        });
        let stdout = '';
        let stderr = '';
        child.stdout.on('data', (chunk) => {
            stdout = (stdout + chunk.toString()).slice(-maxOutput);
        });
        child.stderr.on('data', (chunk) => {
            stderr = (stderr + chunk.toString()).slice(-maxOutput);
        });
        child.on('error', reject);
        child.on('close', (code, signal) => {
            if (code === 0) return resolveRun({ stdout, stderr });
            reject(
                new Error(`${command} ${args.slice(0, 3).join(' ')} failed (${signal ?? code})\n${stderr || stdout}`),
            );
        });
    });
}

async function probe(path) {
    const { stdout } = await run('ffprobe', [
        '-v',
        'error',
        '-count_frames',
        '-show_streams',
        '-show_format',
        '-of',
        'json',
        path,
    ]);
    return JSON.parse(stdout);
}

async function hash(path) {
    const digest = createHash('sha256');
    for await (const chunk of createReadStream(path)) digest.update(chunk);
    return digest.digest('hex');
}

async function fastStart(path) {
    const file = await open(path, 'r');
    try {
        const { size } = await file.stat();
        let offset = 0;
        let sawMoov = false;
        while (offset + 8 <= size) {
            const header = Buffer.alloc(16);
            const { bytesRead } = await file.read(header, 0, 16, offset);
            if (bytesRead < 8) throw new Error(`Truncated MP4 atom in ${path}`);
            const atom = header.toString('ascii', 4, 8);
            let length = header.readUInt32BE(0);
            if (length === 1) {
                if (bytesRead < 16) throw new Error(`Truncated MP4 atom in ${path}`);
                length = Number(header.readBigUInt64BE(8));
            } else if (length === 0) {
                length = size - offset;
            }
            if (!Number.isSafeInteger(length) || length < 8 || offset + length > size) {
                throw new Error(`Invalid MP4 atom ${atom} in ${path}`);
            }
            if (atom === 'moov') sawMoov = true;
            if (atom === 'mdat') return sawMoov;
            offset += length;
        }
        throw new Error(`No media-data atom found in ${path}`);
    } finally {
        await file.close();
    }
}

async function ensureFastStart(path) {
    if (await fastStart(path)) return;
    const temp = `${path}.faststart.mp4`;
    try {
        await run('ffmpeg', [
            '-hide_banner',
            '-loglevel',
            'error',
            '-y',
            '-i',
            path,
            '-map',
            '0',
            '-c',
            'copy',
            '-movflags',
            '+faststart',
            temp,
        ]);
        if (!(await fastStart(temp))) throw new Error(`Faststart remux failed for ${path}`);
        await rename(temp, path);
    } finally {
        await unlink(temp).catch(() => {});
    }
}

function expectedSize(variant, preview) {
    return {
        width: preview ? variant.width / 2 : variant.width,
        height: preview ? variant.height / 2 : variant.height,
    };
}

function assert(condition, message) {
    if (!condition) throw new Error(message);
}

async function verifyVideo(path, variant, preview) {
    const info = await probe(path);
    const video = info.streams.find((stream) => stream.codec_type === 'video');
    const audio = info.streams.find((stream) => stream.codec_type === 'audio');
    const size = expectedSize(variant, preview);
    assert(video && audio, `${path}: video or audio stream missing`);
    assert(
        video.width === size.width && video.height === size.height,
        `${path}: expected ${size.width}x${size.height}, got ${video.width}x${video.height}`,
    );
    assert(video.codec_name === 'h264' && video.pix_fmt === 'yuv420p', `${path}: expected H.264 yuv420p`);
    assert(video.color_space === 'bt709' && video.color_range === 'tv', `${path}: expected limited-range BT.709`);
    assert(audio.codec_name === 'aac' && Number(audio.sample_rate) === 48_000, `${path}: expected 48 kHz AAC`);
    const [numerator, denominator] = video.avg_frame_rate.split('/').map(Number);
    assert(numerator / denominator === frameRate, `${path}: expected ${frameRate} fps`);
    const frames = Number(video.nb_read_frames ?? video.nb_frames);
    assert(frames === frameCount, `${path}: expected ${frameCount} frames, got ${frames}`);
    const duration = Number(info.format.duration);
    assert(Math.abs(duration - durationSeconds) <= 0.15, `${path}: expected ${durationSeconds}s, got ${duration}s`);
    if (audio.duration) {
        assert(
            Math.abs(Number(audio.duration) - durationSeconds) <= 0.15,
            `${path}: audio stream is shorter or longer than ${durationSeconds}s`,
        );
    }
    assert(await fastStart(path), `${path}: MP4 is missing faststart metadata`);

    await run('ffmpeg', [
        '-hide_banner',
        '-loglevel',
        'error',
        '-xerror',
        '-i',
        path,
        '-map',
        '0:v:0',
        '-map',
        '0:a:0',
        '-f',
        'null',
        '-',
    ]);

    const { stderr } = await run('ffmpeg', [
        '-hide_banner',
        '-nostats',
        '-i',
        path,
        '-map',
        '0:a:0',
        '-af',
        'loudnorm=I=-16:TP=-1:LRA=7:print_format=json',
        '-f',
        'null',
        '-',
    ]);
    const measurements = stderr.match(/\{[^{}]*"input_i"[^{}]*\}/g);
    assert(measurements?.length, `${path}: FFmpeg returned no loudness measurement`);
    const loudness = JSON.parse(measurements.at(-1));
    const integratedLufs = Number(loudness.input_i);
    const truePeakDbtp = Number(loudness.input_tp);
    assert(
        Number.isFinite(integratedLufs) && Math.abs(integratedLufs + 16) <= 1,
        `${path}: integrated loudness ${integratedLufs} LUFS is outside -16 ±1 LUFS`,
    );
    assert(
        Number.isFinite(truePeakDbtp) && truePeakDbtp <= -1,
        `${path}: true peak ${truePeakDbtp} dBTP exceeds -1 dBTP`,
    );

    return {
        width: video.width,
        height: video.height,
        fps: frameRate,
        frames,
        durationSeconds: duration,
        videoCodec: video.codec_name,
        pixelFormat: video.pix_fmt,
        colorSpace: video.color_space,
        colorRange: video.color_range,
        audioCodec: audio.codec_name,
        audioSampleRate: Number(audio.sample_rate),
        integratedLufs,
        truePeakDbtp,
        sizeBytes: (await stat(path)).size,
        sha256: await hash(path),
    };
}

async function verifyPoster(path, variant, preview) {
    const png = await readFile(path);
    const signature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
    assert(
        png.subarray(0, 8).equals(signature) && png.toString('ascii', 12, 16) === 'IHDR',
        `${path}: invalid PNG poster`,
    );
    const width = png.readUInt32BE(16);
    const height = png.readUInt32BE(20);
    const size = expectedSize(variant, preview);
    assert(
        width === size.width && height === size.height,
        `${path}: expected ${size.width}x${size.height}, got ${width}x${height}`,
    );
    await run('ffmpeg', ['-v', 'error', '-xerror', '-i', path, '-f', 'null', '-']);
    return { width, height, frame: 65, sizeBytes: png.length, sha256: await hash(path) };
}

async function renderVariant(name, variant, directory, preview) {
    const base = `ShortWin-promo-${name}-${version}`;
    const videoPath = join(directory, `${base}.mp4`);
    const posterPath = join(directory, `${base}-poster.png`);
    const scaleArgs = preview ? ['--scale=0.5'] : [];
    console.log(`Rendering ${variant.composition}${preview ? ' preview' : ''}...`);
    await run('pnpm', [
        'exec',
        'remotion',
        'render',
        entryPoint,
        variant.composition,
        videoPath,
        `--browser-executable=${browser}`,
        '--concurrency=2',
        `--crf=${preview ? 23 : 18}`,
        '--codec=h264',
        '--pixel-format=yuv420p',
        '--image-format=png',
        '--color-space=bt709',
        '--audio-codec=aac',
        '--audio-bitrate=192k',
        '--sample-rate=48000',
        '--overwrite',
        '--quiet',
        ...scaleArgs,
    ]);
    await ensureFastStart(videoPath);
    console.log(`Rendering frame 65 poster for ${variant.composition}...`);
    await run('pnpm', [
        'exec',
        'remotion',
        'still',
        entryPoint,
        variant.composition,
        posterPath,
        `--browser-executable=${browser}`,
        '--frame=65',
        '--overwrite',
        '--quiet',
        ...scaleArgs,
    ]);
    return { videoPath, posterPath };
}

async function main() {
    const options = parseArgs(process.argv.slice(2));
    const directory = options.preview ? join(outputDir, 'draft') : outputDir;
    const selected = options.variant === 'all' ? Object.keys(variants) : [options.variant];
    if (!options.verifyOnly) {
        const soundtrack = join(projectDir, 'public/audio/mix.wav');
        const soundtrackInfo = await probe(soundtrack);
        const audio = soundtrackInfo.streams.find((stream) => stream.codec_type === 'audio');
        assert(audio, `Missing soundtrack: ${soundtrack}`);
        assert(
            Number(audio.sample_rate) === 48_000 &&
                Math.abs(Number(soundtrackInfo.format.duration) - durationSeconds) < 0.01,
            `Soundtrack must be 48 kHz and ${durationSeconds}s: ${soundtrack}`,
        );
        await mkdir(directory, { recursive: true });
    }

    const results = {};
    for (const name of selected) {
        const variant = variants[name];
        const base = `ShortWin-promo-${name}-${version}`;
        const paths = options.verifyOnly
            ? { videoPath: join(directory, `${base}.mp4`), posterPath: join(directory, `${base}-poster.png`) }
            : await renderVariant(name, variant, directory, options.preview);
        console.log(`Verifying ${name} video and poster...`);
        results[name] = {
            composition: variant.composition,
            video: {
                path: relative(projectDir, paths.videoPath),
                ...(await verifyVideo(paths.videoPath, variant, options.preview)),
            },
            poster: {
                path: relative(projectDir, paths.posterPath),
                ...(await verifyPoster(paths.posterPath, variant, options.preview)),
            },
        };
    }

    const manifest = {
        schemaVersion: 1,
        version,
        mode: options.preview ? 'preview' : 'final',
        source: { frames: frameCount, fps: frameRate, durationSeconds },
        verifiedAt: new Date().toISOString(),
        variants: results,
    };
    const manifestPath = join(directory, 'export-manifest.json');
    const tempPath = `${manifestPath}.${process.pid}.tmp`;
    try {
        await writeFile(tempPath, `${JSON.stringify(manifest, null, 2)}\n`);
        await rename(tempPath, manifestPath);
    } finally {
        await unlink(tempPath).catch(() => {});
    }
    console.log(`Verified ${selected.join(', ')}; wrote ${relative(projectDir, manifestPath)}`);
}

main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
});

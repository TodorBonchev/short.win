#!/usr/bin/env node
/** Prepare captured app pixels for the promo. No validator, wallet, or API activity. */
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { copyFile, mkdir, readFile, rename, stat, writeFile } from 'node:fs/promises';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = dirname(fileURLToPath(import.meta.url));
const projectDir = resolve(scriptDir, '..');
const repoDir = resolve(projectDir, '..', '..');
const help =
    'Usage: node tools/promo/scripts/prepare.mjs [--source-dir /path/to/apps/web/.e2e/demo] [--font-file /path/to/font.ttf] [--font-license /path/to/LICENSE]';
const args = process.argv.slice(2);
if (args.includes('--help')) {
    console.log(help);
    process.exit(0);
}
if (Number(process.versions.node.split('.')[0]) < 22) throw new Error('Node.js 22 or newer is required');
const options = new Map();
for (let index = 0; index < args.length; index += 2) {
    const name = args[index];
    const value = args[index + 1];
    if (
        !['--source-dir', '--font-file', '--font-license'].includes(name) ||
        !value ||
        value.startsWith('--') ||
        options.has(name)
    )
        throw new Error(help);
    options.set(name, resolve(value));
}
const sourceDir = options.get('--source-dir') ?? join(repoDir, 'apps', 'web', '.e2e', 'demo');
const fontSource = options.get('--font-file') ?? '/usr/share/fonts/Adwaita/AdwaitaSans-Regular.ttf';
const fontLicenseSource = options.get('--font-license') ?? '/usr/share/licenses/adwaita-fonts/LICENSE';
const footageDir = join(projectDir, 'public', 'footage');
const brandDir = join(projectDir, 'public', 'brand');
const fontsDir = join(projectDir, 'public', 'fonts');
const manifestPath = join(footageDir, 'manifest.json');
const encoding = { codec: 'libx264', preset: 'slow', crf: 16, pixelFormat: 'yuv420p', audio: false };

function run(command, argv) {
    const result = spawnSync(command, argv, { encoding: 'utf8', maxBuffer: 8 * 1024 * 1024 });
    if (result.error) throw new Error(`${command}: ${result.error.message}`);
    if (result.status !== 0) throw new Error(`${command} failed (${result.status}): ${result.stderr.slice(-3000)}`);
    return result.stdout;
}

function probe(file) {
    return JSON.parse(
        run('ffprobe', [
            '-v',
            'error',
            '-show_entries',
            'format=duration,size:stream=codec_type,codec_name,width,height,pix_fmt,avg_frame_rate,duration',
            '-of',
            'json',
            file,
        ]),
    );
}

async function sha256(file) {
    const hash = createHash('sha256');
    for await (const chunk of createReadStream(file)) hash.update(chunk);
    return hash.digest('hex');
}

function verifyOutput(info, expected, sourceDuration) {
    const video = info.streams.filter((stream) => stream.codec_type === 'video');
    if (
        info.streams.length !== 1 ||
        video.length !== 1 ||
        video[0].codec_name !== 'h264' ||
        video[0].width !== expected.width ||
        video[0].height !== expected.height ||
        video[0].pix_fmt !== 'yuv420p' ||
        video[0].avg_frame_rate !== '30/1' ||
        Math.abs(Number(info.format.duration) - sourceDuration) > 0.1
    ) {
        throw new Error(
            `Prepared footage failed dimensions, duration, video-only, or codec checks: ${JSON.stringify(info)}`,
        );
    }
}

await mkdir(footageDir, { recursive: true });
await mkdir(brandDir, { recursive: true });
await mkdir(fontsDir, { recursive: true });
// Fail before encoding if the explicitly distributed font or its license is unavailable.
await stat(fontSource);
await stat(fontLicenseSource);
let previous;
try {
    previous = JSON.parse(await readFile(manifestPath, 'utf8'));
} catch (error) {
    if (error.code !== 'ENOENT') throw error;
}

const definitions = [
    {
        name: 'desktop',
        directory: sourceDir,
        inputWidth: 1440,
        inputHeight: 1040,
        crop: { width: 1440, height: 900, x: 0, y: 0 },
    },
    {
        name: 'phone',
        directory: join(sourceDir, 'phone'),
        inputWidth: 1080,
        inputHeight: 1920,
        crop: { width: 780, height: 1560, x: 150, y: 64 },
    },
];
const assets = {};
for (const definition of definitions) {
    const { name, directory, inputWidth, inputHeight, crop } = definition;
    const renderPath = join(directory, 'render.json');
    const render = JSON.parse(await readFile(renderPath, 'utf8'));
    // Resolve alongside its manifest, so --source-dir also works with a copied artifact set.
    const source = join(directory, basename(render.mp4));
    const sourceInfo = probe(source);
    const video = sourceInfo.streams.find((stream) => stream.codec_type === 'video');
    const sourceDuration = Number(sourceInfo.format.duration);
    if (
        video?.width !== inputWidth ||
        video?.height !== inputHeight ||
        video.avg_frame_rate !== '30/1' ||
        !Number.isFinite(sourceDuration) ||
        sourceDuration <= 0 ||
        Math.abs(sourceDuration - render.duration) > 0.1
    ) {
        throw new Error(`${name}: source does not match the verified master: ${JSON.stringify(sourceInfo)}`);
    }
    const sourceHash = await sha256(source);
    const output = join(footageDir, `${name}.mp4`);
    const cached = previous?.assets?.[name];
    let reused = false;
    if (
        cached?.source?.sha256 === sourceHash &&
        JSON.stringify(cached.crop) === JSON.stringify(crop) &&
        JSON.stringify(previous.encoding) === JSON.stringify(encoding)
    ) {
        try {
            reused = (await stat(output)).isFile() && (await sha256(output)) === cached.output.sha256;
        } catch (error) {
            if (error.code !== 'ENOENT') throw error;
        }
    }
    if (!reused) {
        const temporary = join(footageDir, `${name}.preparing.mp4`);
        console.log(`[prepare] cropping ${name}: ${crop.width}×${crop.height} at (${crop.x}, ${crop.y})`);
        run('ffmpeg', [
            '-hide_banner',
            '-loglevel',
            'error',
            '-y',
            '-i',
            source,
            '-map',
            '0:v:0',
            '-vf',
            `crop=${crop.width}:${crop.height}:${crop.x}:${crop.y},setsar=1`,
            '-c:v',
            encoding.codec,
            '-preset',
            encoding.preset,
            '-crf',
            String(encoding.crf),
            '-pix_fmt',
            encoding.pixelFormat,
            '-an',
            '-sn',
            '-dn',
            '-map_metadata',
            '-1',
            '-movflags',
            '+faststart',
            temporary,
        ]);
        verifyOutput(probe(temporary), crop, sourceDuration);
        run('ffmpeg', ['-hide_banner', '-loglevel', 'error', '-i', temporary, '-map', '0:v:0', '-f', 'null', '-']);
        await rename(temporary, output);
    }
    const outputInfo = probe(output);
    verifyOutput(outputInfo, crop, sourceDuration);
    assets[name] = {
        version: render.version,
        publicPath: `footage/${name}.mp4`,
        source: { path: source, manifest: renderPath, sha256: sourceHash, probe: sourceInfo },
        output: { path: output, sha256: await sha256(output), probe: outputInfo },
        crop,
        timelineUnchanged: true,
        notes: 'Crop of the captured master only. Old guide strips removed. Existing on-screen cards and captured 8× wait intervals remain in this full source; the promo edit must select its documented excerpts.',
    };
    console.log(
        `[prepare] ${name}: ${sourceDuration.toFixed(3)}s ${reused ? '(cached)' : '(encoded and decoded)'} → ${output}`,
    );
}

const brand = {};
for (const name of ['mark.svg', 'lockup-two-tone.svg']) {
    const source = join(repoDir, 'docs', 'design', 'design-pass-2026-10', 'artifacts', 'logo', 'final', name);
    const output = join(brandDir, name);
    await copyFile(source, output);
    const sourceHash = await sha256(source);
    const outputHash = await sha256(output);
    if (sourceHash !== outputHash) throw new Error(`Brand copy mismatch: ${name}`);
    brand[name] = { publicPath: `brand/${name}`, source, output, sha256: outputHash };
}

const fonts = {};
for (const [name, source] of [
    ['AdwaitaSans.ttf', fontSource],
    ['LICENSE', fontLicenseSource],
]) {
    const output = join(fontsDir, name);
    await copyFile(source, output);
    const sourceHash = await sha256(source);
    const outputHash = await sha256(output);
    if (sourceHash !== outputHash) throw new Error(`Font copy mismatch: ${name}`);
    fonts[name] = { publicPath: `fonts/${name}`, source, output, sha256: outputHash };
}

const manifest = {
    schemaVersion: 1,
    generatedAt: new Date().toISOString(),
    sourceDir,
    encoding,
    assets,
    brand,
    fonts,
    reproduction: help.replace(/^Usage: /, ''),
};
const temporaryManifest = `${manifestPath}.preparing`;
await writeFile(temporaryManifest, `${JSON.stringify(manifest, null, 2)}\n`);
await rename(temporaryManifest, manifestPath);
console.log(`[prepare] ready: ${manifestPath}`);

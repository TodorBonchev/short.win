/**
 * Build the 56-second promo soundtrack and aligned captions.
 * node scripts/audio.mjs             # synthesize missing clips once, then render locally
 * node scripts/audio.mjs --offline   # render entirely from the response cache
 * node scripts/audio.mjs --dry-run   # print the script and timing without reading credentials
 *
 * Speech: ElevenLabs Bella; credentials are read only in-process for an uncached request.
 * Music: an original deterministic additive-synthesis arrangement, with no sampled assets.
 */
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const output = join(root, 'public', 'audio');
const cache = join(root, '.cache', 'audio');
const args = new Set(process.argv.slice(2));
const duration = 56;
const sampleRate = 48_000;
const voice = { id: 'hpp4J3VqNfWAUOO0d1Us', name: 'Bella - Professional, Bright, Warm' };
const model = 'eleven_multilingual_v2';
const settings = { stability: 0.6, similarity_boost: 0.75, style: 0, use_speaker_boost: true, speed: 1 };
const scenes = [
    { id: 'hook', start: 0, end: 4, text: 'Rent the coin. Sell the coin. Buy it back.' },
    { id: 'supply', start: 4, end: 11, text: 'Create a market. Supply tokens for borrowers.' },
    {
        id: 'open',
        start: 11,
        end: 23,
        text: 'Choose a size. Review the collateral and liquidation risk. Sale proceeds stay locked. Sign to open a short.',
    },
    { id: 'close', start: 23, end: 31, text: 'To close, buy the tokens back. Review the cost and what returns.' },
    {
        id: 'liquidation',
        start: 31,
        end: 39,
        text: 'On a separate test market, a scripted price rise triggers liquidation.',
    },
    {
        id: 'settlement',
        start: 39,
        end: 49,
        text: 'Here, failed buyback leads to S one. Lenders receive remaining sol, which may be worth less than their tokens.',
    },
    { id: 'outro', start: 49, end: 56, text: 'Explore Short dot win. Inspect the code and reproduce the local demo.' },
];
const hash = (value) => createHash('sha256').update(value).digest('hex');
const save = (file, value) => writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
const json = (file) => JSON.parse(readFileSync(file, 'utf8'));
function run(bin, argv) {
    const result = spawnSync(bin, argv, { encoding: 'utf8', maxBuffer: 8 * 1024 * 1024 });
    if (result.status !== 0) throw new Error(`${bin} failed: ${result.stderr.slice(-2000)}`);
    return result;
}
const ffmpeg = (argv) => run('ffmpeg', ['-hide_banner', '-loglevel', 'error', '-y', ...argv]);
const probe = (file) =>
    JSON.parse(
        run('ffprobe', [
            '-v',
            'error',
            '-show_entries',
            'format=duration:stream=sample_rate,channels,codec_name',
            '-of',
            'json',
            file,
        ]).stdout,
    );
const length = (file) => Number(probe(file).format.duration);

// Never print, persist, fingerprint, or put this credential into command-line arguments.
function apiKey() {
    const raw = process.env.ELEVENLABS_API_KEY ?? readFileSync(join(homedir(), '.11l'), 'utf8');
    const key = raw
        .trim()
        .replace(/^ELEVENLABS_API_KEY\s*=\s*/, '')
        .replace(/^["']|["']$/g, '');
    if (!key || /\s/.test(key)) throw new Error('unsupported ElevenLabs credential format; contents withheld');
    return key;
}

let newRequests = 0;
async function synthesize(scene) {
    const request = { text: scene.text, model_id: model, voice_settings: settings, seed: 103 };
    const cacheKey = hash(JSON.stringify({ voice: voice.id, outputFormat: 'mp3_44100_128', request }));
    const prefix = join(cache, cacheKey);
    const responseFile = `${prefix}.response.json`;
    let response;
    if (existsSync(responseFile)) response = json(responseFile);
    else {
        if (args.has('--offline')) throw new Error(`uncached speech: ${scene.id}; offline mode makes no requests`);
        console.log(`[audio] synthesizing ${scene.id} (${scene.text.length} characters)`);
        let fetched;
        try {
            fetched = await fetch(
                `https://api.elevenlabs.io/v1/text-to-speech/${voice.id}/with-timestamps?output_format=mp3_44100_128`,
                {
                    method: 'POST',
                    headers: { 'xi-api-key': apiKey(), 'Content-Type': 'application/json' },
                    body: JSON.stringify(request),
                    signal: AbortSignal.timeout(120_000),
                    redirect: 'error',
                },
            );
        } catch {
            throw new Error(`speech request failed for ${scene.id}; no automatic paid retry`);
        }
        if (!fetched.ok)
            throw new Error(`speech request failed for ${scene.id}: HTTP ${fetched.status}; no automatic retry`);
        const body = await fetched.json();
        response = {
            audio_base64: body.audio_base64,
            alignment: body.alignment,
            requestId: fetched.headers.get('request-id'),
            reportedCharacterCost: fetched.headers.get('character-cost'),
            requestedCharacters: scene.text.length,
            generatedAt: new Date().toISOString(),
        };
        // Save before any local work so encoding failures never incur a second synthesis.
        save(responseFile, response);
        newRequests++;
    }
    const alignment = response.alignment;
    const count = scene.text.length;
    if (
        !response.audio_base64 ||
        !alignment ||
        alignment.characters.join('') !== scene.text ||
        alignment.character_start_times_seconds.length !== count ||
        alignment.character_end_times_seconds.length !== count
    ) {
        throw new Error(`missing or mismatched character alignment for ${scene.id}`);
    }
    for (let i = 0; i < count; i++) {
        const start = alignment.character_start_times_seconds[i];
        const end = alignment.character_end_times_seconds[i];
        if (
            !Number.isFinite(start) ||
            !Number.isFinite(end) ||
            start < 0 ||
            end < start ||
            (i && start < alignment.character_start_times_seconds[i - 1])
        )
            throw new Error(`invalid character timing for ${scene.id}`);
    }
    const raw = `${prefix}.mp3`;
    writeFileSync(raw, Buffer.from(response.audio_base64, 'base64'));
    const first = alignment.characters.findIndex((character) => /\S/.test(character));
    let last = count - 1;
    while (last >= 0 && !/\S/.test(alignment.characters[last])) last--;
    const trimStart = Math.max(0, alignment.character_start_times_seconds[first] - 0.065);
    const trimEnd = Math.min(length(raw), alignment.character_end_times_seconds[last] + 0.1);
    const available = scene.end - scene.start - 0.35;
    const speed = Math.max(1, Math.ceil(((trimEnd - trimStart) / available) * 10000) / 10000);
    if (speed > 1.12) throw new Error(`${scene.id} needs ${speed.toFixed(3)}× speech; shorten its script instead`);
    const file = `${prefix}.wav`;
    ffmpeg([
        '-i',
        raw,
        '-af',
        `atrim=start=${trimStart}:end=${trimEnd},asetpts=PTS-STARTPTS,atempo=${speed},afade=t=in:d=0.006`,
        '-ar',
        String(sampleRate),
        '-ac',
        '1',
        '-c:a',
        'pcm_s24le',
        file,
    ]);
    const seconds = length(file);
    if (seconds > available + 0.015) throw new Error(`${scene.id} overflows its speech window after speed adjustment`);
    const speech = {
        ...scene,
        voiceStart: scene.start + 0.2,
        voiceEnd: scene.start + 0.2 + seconds,
        duration: seconds,
        trimStart,
        speed,
        cacheKey,
        file,
        alignment,
        requestId: response.requestId,
        reportedCharacterCost: response.reportedCharacterCost,
        requestedCharacters: scene.text.length,
        generatedAt: response.generatedAt,
    };
    save(`${prefix}.json`, speech);
    console.log(`[audio] ${scene.id}: ${seconds.toFixed(2)}s in ${available.toFixed(2)}s, ${speed.toFixed(3)}×`);
    return speech;
}

function canonical(text) {
    return text
        .replace(/\bS one\b/g, 'S1')
        .replace(/\bsol\b/g, 'SOL')
        .replace(/\bShort dot win\b/g, 'short.win');
}
function balance(text) {
    if (text.length <= 29) return text;
    let split = -1;
    for (let i = 0; i < text.length; i++) {
        if (
            text[i] === ' ' &&
            (split === -1 || Math.abs(i - (text.length - i - 1)) < Math.abs(split - (text.length - split - 1)))
        )
            split = i;
    }
    return split < 0 ? text : `${text.slice(0, split)}\n${text.slice(split + 1)}`;
}
function captions(clips) {
    const chunks = [];
    const allWords = [];
    for (const clip of clips) {
        const words = [...clip.text.matchAll(/\S+/g)].map((match) => {
            const first = match.index;
            const last = first + match[0].length - 1;
            const startMs = Math.round(
                1000 *
                    (clip.voiceStart +
                        Math.max(0, clip.alignment.character_start_times_seconds[first] - clip.trimStart) / clip.speed),
            );
            const endMs = Math.round(
                1000 *
                    (clip.voiceStart +
                        (clip.alignment.character_end_times_seconds[last] - clip.trimStart) / clip.speed),
            );
            return { text: match[0], startMs, endMs, timestampMs: Math.round((startMs + endMs) / 2), confidence: null };
        });
        allWords.push(...words.map((word, i) => ({ ...word, text: `${i ? ' ' : ''}${word.text}` })));
        const groups = [];
        const textLength = (entries) => entries.map((word) => word.text).join(' ').length;
        const partition = (entries) => {
            if (!entries.length) return;
            if (textLength(entries) <= 45) {
                groups.push(entries);
                return;
            }
            let split = 1;
            for (let i = 2; i < entries.length; i++) {
                if (
                    Math.abs(textLength(entries.slice(0, i)) - textLength(entries.slice(i))) <
                    Math.abs(textLength(entries.slice(0, split)) - textLength(entries.slice(split)))
                )
                    split = i;
            }
            partition(entries.slice(0, split));
            partition(entries.slice(split));
        };
        let sentence = [];
        for (const word of words) {
            sentence.push(word);
            if (/[.!?]$/.test(word.text)) {
                partition(sentence);
                sentence = [];
            }
        }
        partition(sentence);
        for (const group of groups) {
            const text = canonical(group.map((word) => word.text).join(' '));
            const startMs = group[0].startMs;
            const endMs = group.at(-1).endMs;
            chunks.push({
                text: balance(text),
                startMs,
                endMs,
                timestampMs: Math.round((startMs + endMs) / 2),
                confidence: null,
                pageBreakAfter: true,
            });
        }
    }
    for (const [i, caption] of chunks.entries()) {
        if (
            !(caption.startMs >= 0 && caption.endMs > caption.startMs && caption.endMs <= duration * 1000) ||
            (i > 0 && caption.startMs < chunks[i - 1].endMs) ||
            caption.text.length > 45 ||
            caption.text.split('\n').length > 2
        ) {
            throw new Error(`invalid caption ${i}`);
        }
    }
    save(join(output, 'captions.json'), chunks);
    save(join(output, 'alignedwords.json'), allWords);
    return { chunks: chunks.length, words: allWords.length };
}

function measure(file) {
    const result = run('ffmpeg', [
        '-hide_banner',
        '-i',
        file,
        '-af',
        'loudnorm=I=-16:TP=-1.6:LRA=7:print_format=json',
        '-f',
        'null',
        '-',
    ]);
    const text = result.stderr;
    const data = JSON.parse(text.slice(text.lastIndexOf('{'), text.lastIndexOf('}') + 1));
    return {
        data,
        integratedLufs: Number(data.input_i),
        truePeakDb: Number(data.input_tp),
        loudnessRangeLu: Number(data.input_lra),
    };
}
function normalize(source, destination, target = -16, peak = -1.6) {
    const first = run('ffmpeg', [
        '-hide_banner',
        '-i',
        source,
        '-af',
        `loudnorm=I=${target}:TP=${peak}:LRA=7:print_format=json`,
        '-f',
        'null',
        '-',
    ]);
    const text = first.stderr;
    const m = JSON.parse(text.slice(text.lastIndexOf('{'), text.lastIndexOf('}') + 1));
    const filter = `loudnorm=I=${target}:TP=${peak}:LRA=7:measured_I=${m.input_i}:measured_TP=${m.input_tp}:measured_LRA=${m.input_lra}:measured_thresh=${m.input_thresh}:offset=${m.target_offset}:linear=true`;
    ffmpeg(['-i', source, '-af', filter, '-ar', String(sampleRate), '-ac', '2', '-c:a', 'pcm_s24le', destination]);
    const measured = measure(destination);
    if (Math.abs(measured.integratedLufs - target) > 0.35 || measured.truePeakDb > peak + 0.1)
        throw new Error(`unexpected loudness: ${destination}`);
    return {
        integratedLufs: measured.integratedLufs,
        truePeakDb: measured.truePeakDb,
        loudnessRangeLu: measured.loudnessRangeLu,
    };
}

/** Original, deterministic four-voicing ambient loop; no samples, external tracks or melody quotes. */
function synthMusic(file) {
    const frames = sampleRate * duration;
    const bytes = Buffer.alloc(44 + frames * 4);
    bytes.write('RIFF', 0);
    bytes.writeUInt32LE(bytes.length - 8, 4);
    bytes.write('WAVEfmt ', 8);
    bytes.writeUInt32LE(16, 16);
    bytes.writeUInt16LE(1, 20);
    bytes.writeUInt16LE(2, 22);
    bytes.writeUInt32LE(sampleRate, 24);
    bytes.writeUInt32LE(sampleRate * 4, 28);
    bytes.writeUInt16LE(4, 32);
    bytes.writeUInt16LE(16, 34);
    bytes.write('data', 36);
    bytes.writeUInt32LE(frames * 4, 40);
    const hz = (midi) => 440 * 2 ** ((midi - 69) / 12);
    const voicings = [
        [48, 55, 58, 63, 67],
        [44, 55, 60, 63, 70],
        [51, 58, 62, 65, 67],
        [46, 53, 57, 60, 65],
    ];
    const chordSeconds = 5; // 96 BPM, two bars per voicing.
    const notes = [];
    for (let bar = 0; bar < Math.ceil(duration / chordSeconds); bar++) {
        for (const [index, midi] of voicings[bar % voicings.length].entries())
            notes.push({
                start: bar * chordSeconds,
                end: Math.min(duration, (bar + 1) * chordSeconds + 0.7),
                frequency: hz(midi),
                pan: (index - 2) * 0.25,
            });
    }
    const tau = 2 * Math.PI;
    for (let i = 0; i < frames; i++) {
        const t = i / sampleRate;
        let left = 0;
        let right = 0;
        for (const note of notes) {
            if (t < note.start || t >= note.end) continue;
            const age = t - note.start;
            const envelope = Math.min(1, age / 0.8) * Math.min(1, (note.end - t) / 0.8) * 0.022;
            const a = Math.sin(tau * note.frequency * age) + 0.16 * Math.sin(tau * note.frequency * 2 * age);
            const b =
                Math.sin(tau * note.frequency * 1.0007 * age) + 0.16 * Math.sin(tau * note.frequency * 2.0007 * age);
            left += envelope * a * Math.sqrt((1 - note.pan) / 2);
            right += envelope * b * Math.sqrt((1 + note.pan) / 2);
        }
        const beat = Math.floor(t / 0.625);
        const phase = t - beat * 0.625;
        const chord = voicings[Math.floor(t / chordSeconds) % voicings.length];
        const melody = chord[[0, 2, 4, 1, 3, 2, 1, 4][beat % 8]] + 12;
        const pluck = Math.sin(tau * hz(melody) * phase) * Math.min(1, phase / 0.012) * Math.exp(-phase * 10) * 0.018;
        const fade = Math.min(1, t / 1.4, (duration - t) / 2.4);
        left = (left + pluck * 0.55) * fade;
        right = (right + pluck * 0.45) * fade;
        bytes.writeInt16LE(Math.round(Math.max(-1, Math.min(1, left)) * 32767), 44 + i * 4);
        bytes.writeInt16LE(Math.round(Math.max(-1, Math.min(1, right)) * 32767), 46 + i * 4);
    }
    writeFileSync(file, bytes);
}

if (args.has('--dry-run')) {
    console.log(
        JSON.stringify(
            {
                duration,
                voice,
                model,
                settings,
                scenes,
                requestedCharacters: scenes.reduce((n, scene) => n + scene.text.length, 0),
            },
            null,
            2,
        ),
    );
} else {
    mkdirSync(cache, { recursive: true });
    mkdirSync(output, { recursive: true });
    const clips = [];
    for (const scene of scenes) clips.push(await synthesize(scene));
    const captionInfo = captions(clips);
    const voiceRaw = join(cache, 'voice-raw.wav');
    const filters = clips.map((clip, i) => `[${i}:a]adelay=${Math.round(clip.voiceStart * 1000)}:all=1[a${i}]`);
    filters.push(
        `${clips.map((_, i) => `[a${i}]`).join('')}amix=inputs=${clips.length}:normalize=0:dropout_transition=0,apad,atrim=duration=${duration}[voice]`,
    );
    ffmpeg([
        ...clips.flatMap((clip) => ['-i', clip.file]),
        '-filter_complex',
        filters.join(';'),
        '-map',
        '[voice]',
        '-ar',
        String(sampleRate),
        '-c:a',
        'pcm_s24le',
        voiceRaw,
    ]);
    const voicePath = join(output, 'voice.wav');
    const voiceMetrics = normalize(voiceRaw, voicePath);
    const musicRaw = join(cache, 'music-raw.wav');
    synthMusic(musicRaw);
    const musicPath = join(output, 'music.wav');
    // loudnorm's minimum supported true-peak target is -9 dBTP; the quiet -32 LUFS
    // music naturally peaks well below that ceiling.
    const musicMetrics = normalize(musicRaw, musicPath, -32, -9);
    const mixRaw = join(cache, 'mix-raw.wav');
    ffmpeg([
        '-i',
        musicPath,
        '-i',
        voicePath,
        '-filter_complex',
        '[1:a]asplit=2[voice][key];[0:a][key]sidechaincompress=threshold=0.008:ratio=5:attack=12:release=380:makeup=1[bed];[voice][bed]amix=inputs=2:normalize=0:dropout_transition=0,apad,atrim=duration=56[mix]',
        '-map',
        '[mix]',
        '-ar',
        String(sampleRate),
        '-ac',
        '2',
        '-c:a',
        'pcm_s24le',
        mixRaw,
    ]);
    const mixPath = join(output, 'mix.wav');
    const mixMetrics = normalize(mixRaw, mixPath);
    const files = {};
    for (const [name, file, metrics] of [
        ['voice', voicePath, voiceMetrics],
        ['music', musicPath, musicMetrics],
        ['mix', mixPath, mixMetrics],
    ]) {
        const info = probe(file);
        if (
            Math.abs(Number(info.format.duration) - duration) > 0.001 ||
            info.streams[0].sample_rate !== String(sampleRate) ||
            info.streams[0].channels !== 2
        )
            throw new Error(`unexpected format for ${name}`);
        ffmpeg(['-i', file, '-f', 'null', '-']);
        files[name] = {
            filename: `${name}.wav`,
            duration: Number(info.format.duration),
            sampleRate,
            channels: 2,
            sha256: hash(readFileSync(file)),
            ...metrics,
        };
    }
    const manifest = {
        duration,
        voice,
        model,
        settings,
        generatedAt: new Date().toISOString(),
        files,
        captions: captionInfo,
        usage: {
            newRequestsThisRun: newRequests,
            uniqueSpeechRequests: clips.length,
            requestedCharacters: scenes.reduce((n, scene) => n + scene.text.length, 0),
            reportedCharacterCost: clips.reduce((n, clip) => n + Number(clip.reportedCharacterCost ?? 0), 0),
            allCostsReported: clips.every(
                (clip) => clip.reportedCharacterCost !== null && clip.reportedCharacterCost !== undefined,
            ),
            noAutomaticPaidRetries: true,
        },
        music: {
            title: 'ShortWin Quiet Circuit',
            source: 'Original deterministic additive synthesis in scripts/audio.mjs',
            samplesUsed: false,
            bpm: 96,
            voicings: 'Cm9 / Abmaj9 / Ebmaj9 / Bbmaj9',
            ducking: { threshold: 0.008, ratio: 5, attackMs: 12, releaseMs: 380 },
        },
        clips: clips.map(({ file, alignment, ...clip }) => clip),
    };
    save(join(output, 'manifest.json'), manifest);
    console.log(JSON.stringify({ output, files, captions: captionInfo, newRequests }, null, 2));
}

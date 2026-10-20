/**
 * Add ElevenLabs narration to rendered demos without changing their video stream.
 * node e2e/demo/narrate.ts --all             # desktop + phone; shared clips are cached
 * node e2e/demo/narrate.ts --all --dry-run   # inspect scripts/timing without API calls
 * node e2e/demo/narrate.ts --all --offline   # reuse cached speech without credentials
 */
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { basename, join } from 'node:path';
import { E2E_DIR } from '../env.ts';

type Render = {
    version: string;
    duration: number;
    mp4: string;
    chapters: { step: string; seconds: number }[];
    cues: { text: string; start: number }[];
    waits: { kind: string; outputStart: number; outputEnd: number }[];
};
type Alignment = {
    characters: string[];
    character_start_times_seconds: number[];
    character_end_times_seconds: number[];
};
type Clip = { id: string; text: string; start: number; deadline: number };
type Speech = {
    id: string;
    text: string;
    file: string;
    duration: number;
    trimStart: number;
    alignment: Alignment;
    cacheKey: string;
};
const args = new Set(process.argv.slice(2));
const variants = args.has('--all') ? ['desktop', 'phone'] : [args.has('--phone') ? 'phone' : 'desktop'];
const cache = join(E2E_DIR, 'demo', 'narration-cache');
const voice = { id: 'hpp4J3VqNfWAUOO0d1Us', name: 'Bella - Professional, Bright, Warm' };
const model = 'eleven_multilingual_v2';
const settings = { stability: 0.6, similarity_boost: 0.75, style: 0, use_speaker_boost: true, speed: 1 };
const hash = (value: string | Buffer): string => createHash('sha256').update(value).digest('hex');
const json = <T>(file: string): T => JSON.parse(readFileSync(file, 'utf8')) as T;
const save = (file: string, value: unknown): void => writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
const run = (bin: string, argv: string[]) => {
    const result = spawnSync(bin, argv, { encoding: 'utf8', maxBuffer: 8 * 1024 * 1024 });
    if (result.status !== 0) throw new Error(`${bin} failed: ${result.stderr.slice(-2500)}`);
    return result;
};
const probe = (file: string) =>
    JSON.parse(run('ffprobe', ['-v', 'error', '-show_streams', '-show_format', '-of', 'json', file]).stdout) as {
        format: { duration: string };
        streams: { codec_type: string; codec_name: string; width?: number; height?: number; duration?: string }[];
    };
const duration = (file: string): number => Number(probe(file).format.duration);
const ffmpeg = (argv: string[]) => run('ffmpeg', ['-hide_banner', '-loglevel', 'error', '-y', ...argv]);

function storyboard(render: Render): Clip[] {
    const cue = (prefix: string): number => {
        const match = render.cues.find((entry) => entry.text.startsWith(prefix));
        if (!match) throw new Error(`missing rendered action cue: ${prefix}`);
        return match.start;
    };
    const chapter = (step: string): number => {
        const match = render.chapters.find((entry) => entry.step === step);
        if (!match) throw new Error(`missing chapter: ${step}`);
        return match.seconds;
    };
    const warmup = render.waits.find((wait) => wait.kind === 'warmup');
    if (!warmup) throw new Error('missing warm-up interval');
    const borrowerRisk = cue('Review the borrowed tokens') + 1.2;
    const lenderResult = render.cues.find((entry) => entry.text.startsWith('Lenders’ SOL'))?.start;
    const clips: Clip[] = [
        {
            id: 'intro',
            text: `Short dot win. Version ${render.version
                .split('.')
                .map(
                    (n) =>
                        ['zero', 'one', 'two', 'three', 'four', 'five', 'six', 'seven', 'eight', 'nine'][Number(n)] ??
                        n,
                )
                .join(' point ')}. Local validator; not a deployment.`,
            start: 0.2,
            deadline: cue('Create market ·') - 0.25,
        },
        {
            id: 'create',
            text: 'Create a market for this Pump dot fun token. Review the simulation, then sign.',
            start: cue('Create market ·') + 0.05,
            deadline: chapter('deposit') - 0.25,
        },
        {
            id: 'deposit',
            text: 'The lender supplies sixty million tokens, making inventory available for borrowers.',
            start: chapter('deposit') + 0.25,
            deadline: cue('Acknowledge that repayment') + 0.6,
        },
        {
            id: 'lender-risk',
            text: 'Review the shares and confirm the disclosure: repayment may be in sol.',
            start: cue('Deposit ·') + 0.3,
            deadline: cue('Sign in wallet · confirm the token deposit') + 1.3,
        },
        {
            id: 'waits',
            text: 'Waits play at eight times speed.',
            start: warmup.outputStart + 0.1,
            deadline: warmup.outputEnd - 0.15,
        },
        {
            id: 'open',
            text: 'Borrow one point five million tokens and sell them on the curve. Sale proceeds stay locked as collateral.',
            start: chapter('open-curve-short') + 0.3,
            deadline: borrowerRisk - 0.35,
        },
        {
            id: 'borrower-risk',
            text: 'Review the collateral and liquidation risk before signing.',
            start: borrowerRisk,
            deadline: cue('Sign in wallet · open the curve short') + 1.3,
        },
        {
            id: 'close',
            text: 'After a price move, close the short. The preview shows the buyback cost and what returns to the borrower.',
            start: chapter('close-curve-short') + 0.3,
            deadline: cue('Short closed ·') + 0.2,
        },
        {
            id: 'liquidation',
            text: 'Now, a separate, test-only mock market. A scripted price rise triggers a keeper liquidation.',
            start: chapter('scripted-liquidation') + 0.3,
            deadline: cue('Liquidation recorded ·') - 0.1,
        },
        {
            id: 'liquidation-result',
            text: 'The history records the liquidation and keeper payment.',
            start: cue('Liquidation recorded ·') + 0.3,
            deadline: chapter('s1-settlement') - 0.25,
        },
        {
            id: 's1',
            text: 'In another test market, a larger price gap prevents token buyback. S one settlement allocates the remaining sol to lenders.',
            start: chapter('s1-settlement') + 0.3,
            deadline: cue('S1 settlement recorded ·') - 0.35,
        },
        {
            id: 's1-result',
            text: 'The record shows S one and the lenders’ sol.',
            start: (lenderResult ?? cue('S1 settlement recorded ·')) + 0.3,
            deadline: cue('Six confirmed actions ·') - 0.25,
        },
        {
            id: 'outro',
            text: 'Six confirmed actions, all on the local validator.',
            start: cue('Six confirmed actions ·') + 0.2,
            deadline: render.duration - 0.2,
        },
    ];
    for (const [i, clip] of clips.entries()) {
        if (
            !Number.isFinite(clip.start) ||
            !Number.isFinite(clip.deadline) ||
            clip.start < 0 ||
            clip.deadline <= clip.start ||
            clip.deadline > render.duration ||
            (i > 0 && clip.start < clips[i - 1].deadline)
        )
            throw new Error(`invalid or overlapping narration window: ${clip.id}`);
    }
    return clips;
}

// Read the credential only when a new synthesis is needed. Never log it, include it in
// cache fingerprints, pass it through shell arguments, or persist it in generated metadata.
function apiKey(): string {
    const raw = process.env.ELEVENLABS_API_KEY ?? readFileSync(join(homedir(), '.11l'), 'utf8');
    const key = raw
        .trim()
        .replace(/^ELEVENLABS_API_KEY\s*=\s*/, '')
        .replace(/^["']|["']$/g, '');
    if (!key || /\s/.test(key)) throw new Error('unsupported ElevenLabs credential format; contents withheld');
    return key;
}

async function synthesize(clip: Clip): Promise<Speech> {
    const request = { text: clip.text, model_id: model, voice_settings: settings, seed: 103 };
    const cacheKey = hash(JSON.stringify({ voice: voice.id, outputFormat: 'mp3_44100_128', request }));
    const prefix = join(cache, cacheKey);
    const metadata = `${prefix}.json`;
    if (existsSync(metadata)) {
        const cached = json<Speech>(metadata);
        if (cached.cacheKey !== cacheKey || !existsSync(cached.file)) throw new Error(`invalid cache: ${clip.id}`);
        return cached;
    }
    const responseFile = `${prefix}.response.json`;
    type Response = {
        audio_base64: string;
        alignment: Alignment | null;
        requestId: string | null;
        generatedAt: string;
    };
    let result: Response;
    if (existsSync(responseFile)) result = json<Response>(responseFile);
    else {
        if (args.has('--offline'))
            throw new Error(`missing cached speech for ${clip.id}; offline mode makes no API calls`);
        console.log(`[narration] synthesize ${clip.id} (${clip.text.length} characters)`);
        const response = await fetch(
            `https://api.elevenlabs.io/v1/text-to-speech/${voice.id}/with-timestamps?output_format=mp3_44100_128`,
            {
                method: 'POST',
                headers: { 'xi-api-key': apiKey(), 'Content-Type': 'application/json' },
                body: JSON.stringify(request),
                signal: AbortSignal.timeout(120_000),
                redirect: 'error',
            },
        );
        // No automatic retries of a charged request. Save the response before local processing
        // so a failed encode/validation can be retried without synthesizing the speech again.
        if (!response.ok) throw new Error(`ElevenLabs synthesis failed for ${clip.id}: HTTP ${response.status}`);
        result = {
            ...((await response.json()) as Pick<Response, 'audio_base64' | 'alignment'>),
            requestId: response.headers.get('request-id'),
            generatedAt: new Date().toISOString(),
        };
        save(responseFile, result);
    }
    const alignment = result.alignment;
    if (!result.audio_base64 || !alignment || alignment.characters.join('') !== clip.text) {
        throw new Error(`missing or mismatched speech alignment for ${clip.id}`);
    }
    const raw = `${prefix}.mp3`;
    writeFileSync(raw, Buffer.from(result.audio_base64, 'base64'));
    const rawDuration = duration(raw);
    const first = alignment.characters.findIndex((character) => /\S/.test(character));
    let last = alignment.characters.length - 1;
    while (last >= 0 && !/\S/.test(alignment.characters[last])) last--;
    const trimStart = Math.max(0, alignment.character_start_times_seconds[first] - 0.08);
    const trimEnd = Math.min(rawDuration, alignment.character_end_times_seconds[last] + 0.12);
    if (!Number.isFinite(trimEnd) || trimEnd <= trimStart) throw new Error(`invalid audio timing for ${clip.id}`);
    const file = `${prefix}.wav`;
    ffmpeg([
        '-i',
        raw,
        '-af',
        `atrim=start=${trimStart}:end=${trimEnd},asetpts=PTS-STARTPTS,afade=t=in:d=0.008`,
        '-ar',
        '48000',
        '-ac',
        '1',
        '-c:a',
        'pcm_s24le',
        file,
    ]);
    const speech = { id: clip.id, text: clip.text, file, duration: duration(file), trimStart, alignment, cacheKey };
    save(metadata, {
        ...speech,
        voice,
        model,
        settings,
        requestId: result.requestId,
        generatedAt: result.generatedAt,
    });
    return speech;
}

function subtitles(clips: (Clip & { speech: Speech })[]): string {
    const timestamp = (seconds: number): string => {
        const ms = Math.round(seconds * 1000);
        return `${String(Math.floor(ms / 3600000)).padStart(2, '0')}:${String(Math.floor(ms / 60000) % 60).padStart(2, '0')}:${String(Math.floor(ms / 1000) % 60).padStart(2, '0')},${String(ms % 1000).padStart(3, '0')}`;
    };
    const captions: string[] = [];
    for (const clip of clips) {
        const alignment = clip.speech.alignment;
        const words = [...clip.text.matchAll(/\S+/g)];
        let group: typeof words = [];
        const flush = () => {
            if (!group.length) return;
            const first = group[0].index;
            const last = group[group.length - 1].index + group[group.length - 1][0].length - 1;
            const start =
                clip.start + Math.max(0, alignment.character_start_times_seconds[first] - clip.speech.trimStart);
            const end = clip.start + alignment.character_end_times_seconds[last] - clip.speech.trimStart;
            const lines = [''];
            for (const word of group) {
                const i = lines.length - 1;
                if (lines[i] && lines[i].length + word[0].length + 1 > 42) lines.push(word[0]);
                else lines[i] += `${lines[i] ? ' ' : ''}${word[0]}`;
            }
            captions.push(`${captions.length + 1}\n${timestamp(start)} --> ${timestamp(end)}\n${lines.join('\n')}\n`);
            group = [];
        };
        // Keep sentences together and balance long ones, avoiding a one-word subtitle
        // left over after a nearly full preceding caption.
        const groups: (typeof words)[] = [];
        const length = (entries: typeof words) => entries.map((entry) => entry[0]).join(' ').length;
        const append = (entries: typeof words): void => {
            if (!entries.length) return;
            if (length(entries) > 76) {
                let split = 1;
                for (let i = 2; i < entries.length; i++) {
                    if (
                        Math.abs(length(entries.slice(0, i)) - length(entries.slice(i))) <
                        Math.abs(length(entries.slice(0, split)) - length(entries.slice(split)))
                    )
                        split = i;
                }
                append(entries.slice(0, split));
                append(entries.slice(split));
            } else {
                const prior = groups[groups.length - 1];
                if (prior && length([...prior, ...entries]) <= 76) prior.push(...entries);
                else groups.push(entries);
            }
        };
        let sentence: typeof words = [];
        for (const word of words) {
            sentence.push(word);
            if (/[.!?]$/.test(word[0])) {
                append(sentence);
                sentence = [];
            }
        }
        append(sentence);
        for (const entries of groups) {
            group = entries;
            flush();
        }
    }
    return captions.join('\n');
}

const plans = variants.map((variant) => {
    const dir = join(E2E_DIR, 'demo', ...(variant === 'phone' ? ['phone'] : []));
    const render = json<Render>(join(dir, 'render.json'));
    if (!existsSync(render.mp4)) throw new Error(`missing rendered video: ${render.mp4}`);
    return { variant, dir, render, clips: storyboard(render) };
});
if (args.has('--dry-run')) {
    console.log(JSON.stringify({ voice, model, settings, plans }, null, 2));
} else {
    mkdirSync(cache, { recursive: true });
    const shared = new Map<string, Speech>();
    for (const plan of plans) {
        for (const clip of plan.clips) {
            if (!shared.has(clip.text)) {
                const speech = await synthesize(clip);
                const shortestWindow = Math.min(
                    ...plans
                        .flatMap((entry) => entry.clips)
                        .filter((entry) => entry.text === clip.text)
                        .map((entry) => entry.deadline - entry.start),
                );
                if (speech.duration > shortestWindow) {
                    throw new Error(
                        `${clip.id}: ${speech.duration.toFixed(2)}s exceeds the shortest ${shortestWindow.toFixed(2)}s window; shorten the script before generating more clips`,
                    );
                }
                shared.set(clip.text, speech);
            }
        }
    }
    for (const plan of plans) {
        const clips = plan.clips.map((clip) => ({ ...clip, speech: shared.get(clip.text) as Speech }));
        for (const clip of clips) {
            if (clip.start + clip.speech.duration > clip.deadline) {
                throw new Error(
                    `${plan.variant}/${clip.id}: ${clip.speech.duration.toFixed(2)}s speech exceeds ${(clip.deadline - clip.start).toFixed(2)}s window; shorten the script, never speed up or cut speech`,
                );
            }
        }
        const prefix = plan.render.mp4.replace(/\.mp4$/, '-narrated');
        const wav = `${prefix}.wav`;
        const srt = `${prefix}.srt`;
        const output = `${prefix}.mp4`;
        const mixed = join(plan.dir, 'narration-mix.wav');
        const filter = clips.map((clip, i) => `[${i}:a]adelay=${Math.round(clip.start * 1000)}:all=1[a${i}]`);
        filter.push(
            `${clips.map((_, i) => `[a${i}]`).join('')}amix=inputs=${clips.length}:normalize=0:dropout_transition=0,apad,atrim=duration=${plan.render.duration}[voice]`,
        );
        ffmpeg([
            ...clips.flatMap((clip) => ['-i', clip.speech.file]),
            '-filter_complex',
            filter.join(';'),
            '-map',
            '[voice]',
            '-c:a',
            'pcm_s24le',
            mixed,
        ]);
        // Two-pass normalization preserves natural speech speed and leaves headroom for AAC.
        const loudness = run('ffmpeg', [
            '-hide_banner',
            '-i',
            mixed,
            '-af',
            'loudnorm=I=-16:TP=-1.5:LRA=7:print_format=json',
            '-f',
            'null',
            '-',
        ]);
        const measurements = JSON.parse(
            loudness.stderr.slice(loudness.stderr.lastIndexOf('{'), loudness.stderr.lastIndexOf('}') + 1),
        ) as Record<string, string>;
        const normalization = `loudnorm=I=-16:TP=-1.5:LRA=7:measured_I=${measurements.input_i}:measured_TP=${measurements.input_tp}:measured_LRA=${measurements.input_lra}:measured_thresh=${measurements.input_thresh}:offset=${measurements.target_offset}:linear=true`;
        ffmpeg(['-i', mixed, '-af', normalization, '-ar', '48000', '-ac', '1', '-c:a', 'pcm_s24le', wav]);
        writeFileSync(srt, subtitles(clips));
        const temporary = `${prefix}.rendering.mp4`;
        ffmpeg([
            '-i',
            plan.render.mp4,
            '-i',
            wav,
            '-map',
            '0:v:0',
            '-map',
            '1:a:0',
            '-c:v',
            'copy',
            '-c:a',
            'aac',
            '-b:a',
            '192k',
            '-metadata:s:a:0',
            'language=eng',
            '-metadata:s:a:0',
            'title=English narration (ElevenLabs Bella)',
            '-movflags',
            '+faststart',
            temporary,
        ]);
        const info = probe(temporary);
        const videoHash = (file: string) =>
            ffmpeg(['-i', file, '-map', '0:v:0', '-c', 'copy', '-f', 'hash', '-hash', 'sha256', '-']).stdout.trim();
        const originalVideoHash = videoHash(plan.render.mp4);
        if (
            videoHash(temporary) !== originalVideoHash ||
            Math.abs(Number(info.format.duration) - plan.render.duration) > 0.1 ||
            info.streams.length !== 2 ||
            !info.streams.some((stream) => stream.codec_type === 'audio' && stream.codec_name === 'aac')
        )
            throw new Error(`invalid narrated output for ${plan.variant}`);
        ffmpeg(['-i', temporary, '-f', 'null', '-']);
        renameSync(temporary, output);
        save(join(plan.dir, 'narration.json'), {
            variant: plan.variant,
            voice,
            model,
            settings,
            source: plan.render.mp4,
            sourceSha256: hash(readFileSync(plan.render.mp4)),
            output,
            outputSha256: hash(readFileSync(output)),
            duration: Number(info.format.duration),
            videoStreamUnchanged: true,
            videoStreamHash: originalVideoHash,
            normalization: { integratedLufs: -16, truePeakDb: -1.5 },
            wav,
            srt,
            clips: clips.map(({ speech, ...clip }) => ({
                ...clip,
                duration: speech.duration,
                end: clip.start + speech.duration,
                cacheKey: speech.cacheKey,
            })),
        });
        console.log(
            `[narration] ready: ${basename(output)} (${info.format.duration}s; original video stream preserved)`,
        );
    }
}

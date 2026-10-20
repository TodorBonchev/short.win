/** Render the guided one-take capture, accelerating only explicitly recorded waits. */

import { spawnSync } from 'node:child_process';
import { readFileSync, renameSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { E2E_DIR } from '../env.ts';

type Wait = {
    kind: 'warmup' | 'liquidation' | 'settlement';
    start: number;
    end: number;
    speed: number;
};
type Segment = { start: number; end: number; speed: number };
const phone = process.argv.includes('--phone');
const dir = join(E2E_DIR, 'demo', ...(phone ? ['phone'] : []));
const manifest = JSON.parse(readFileSync(join(dir, 'recording.json'), 'utf8')) as {
    version: string;
    variant?: 'phone' | 'desktop';
    viewport?: { width: number; height: number };
    failure: string | null;
    marks: { step: string; seconds: number }[];
    signatures: Record<string, string>;
    pageErrors: string[];
    pointer: boolean;
    cues: { text: string; seconds: number }[];
    waits: Wait[];
};

// An older capture has no visible cursor or precise wait boundaries. Never publish a newly
// labelled cut of that capture as though it had the new guided interactions.
if (manifest.failure) throw new Error(`recording failed: ${manifest.failure}`);
if (manifest.pointer !== true || !manifest.cues?.length || !manifest.waits?.length) {
    throw new Error('record a new guided demo with pointer, cues and exact waits before rendering');
}
if (!Array.isArray(manifest.pageErrors) || manifest.pageErrors.length) {
    throw new Error(`recording has missing or nonempty pageErrors: ${JSON.stringify(manifest.pageErrors)}`);
}
for (const step of ['create', 'deposit', 'open', 'close', 'liquidation', 's1']) {
    if (!manifest.signatures[step]) throw new Error(`missing ${step} signature`);
}
if (!/^\d+\.\d+\.\d+(?:[-+][\w.-]+)?$/.test(manifest.version)) {
    throw new Error('recording version is not a valid release label');
}
if (phone && (manifest.variant !== 'phone' || manifest.viewport?.width !== 390 || manifest.viewport?.height !== 780)) {
    throw new Error('phone rendering requires a phone manifest with a 390×780 touch viewport');
}
if (!phone && manifest.variant === 'phone') throw new Error('use --phone to render a phone capture');
const run = (bin: string, args: string[]): string => {
    const result = spawnSync(bin, args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'] });
    if (result.status !== 0) throw new Error(`${bin} failed: ${result.status ?? result.signal}`);
    return result.stdout.trim();
};
const probe = (file: string) =>
    JSON.parse(
        run('ffprobe', [
            '-v',
            'error',
            '-select_streams',
            'v:0',
            '-show_entries',
            'format=duration:stream=width,height',
            '-of',
            'json',
            file,
        ]),
    ) as { format: { duration: string }; streams: { width: number; height: number }[] };
const raw = join(dir, 'raw.webm');
const rawInfo = probe(raw);
const duration = Number(rawInfo.format.duration);
if (!Number.isFinite(duration) || duration <= 0) throw new Error('raw capture has no valid duration');
const rawWidth = rawInfo.streams[0]?.width;
const rawHeight = rawInfo.streams[0]?.height;
if (phone && !((rawWidth === 390 && rawHeight === 780) || (rawWidth === 780 && rawHeight === 1560))) {
    throw new Error('phone capture must be 390×780 or 780×1560; the whole viewport remains visible');
}
if (!phone && (rawWidth !== 1440 || rawHeight !== 900)) {
    throw new Error('guided capture must be 1440×900; the footer is added below the whole viewport');
}
// Chromium starts its screencast asynchronously, so the saved video's zero can differ from
// the recorder's wall clock. Anchor that clock to the observed final title card instead of
// allowing every click cue and accelerated wait to lag the action by the startup delay.
const outroMark = manifest.marks.find((entry) => entry.step === 'outro')?.seconds;
if (outroMark === undefined || !Number.isFinite(outroMark)) throw new Error('missing outro timing anchor');
const sampleStart = Math.max(0, duration - 8);
const sampleRate = 30;
const headerBytes = 144 * 4;
const headerCropHeight = phone ? (rawWidth / 390) * 40 : 40;
const sample = spawnSync(
    'ffmpeg',
    [
        '-hide_banner',
        '-loglevel',
        'error',
        '-ss',
        sampleStart.toFixed(4),
        '-i',
        raw,
        '-vf',
        `crop=${rawWidth}:${headerCropHeight}:0:0,scale=144:4,format=gray,fps=${sampleRate}`,
        '-an',
        '-f',
        'rawvideo',
        'pipe:1',
    ],
    { stdio: ['ignore', 'pipe', 'inherit'] },
);
if (sample.status !== 0) throw new Error('could not read video timing anchor');
const headerFrames = Math.floor(sample.stdout.length / headerBytes);
const uniformHeader = (frame: number): boolean => {
    const pixels = sample.stdout.subarray(frame * headerBytes, (frame + 1) * headerBytes);
    const mean = pixels.reduce((total, pixel) => total + pixel, 0) / pixels.length;
    const variance = pixels.reduce((total, pixel) => total + (pixel - mean) ** 2, 0) / pixels.length;
    return mean >= 5 && mean <= 35 && variance < 1;
};
let titleFrame = headerFrames;
while (titleFrame > 0 && uniformHeader(titleFrame - 1)) titleFrame--;
if (headerFrames - titleFrame < sampleRate / 2 || titleFrame === 0) {
    throw new Error('could not identify the final title-card transition for timing calibration');
}
const observedOutro = sampleStart + titleFrame / sampleRate;
const wallClockOffset = outroMark - observedOutro;
if (Math.abs(wallClockOffset) > 5) {
    throw new Error(`unexpected screencast startup offset ${wallClockOffset.toFixed(3)}s`);
}
const videoTime = (wallClockSeconds: number): number => Math.max(0, wallClockSeconds - wallClockOffset);
console.log(`[demo] aligned recorder clock to outro: subtract ${wallClockOffset.toFixed(3)}s`);
const at = (step: string): number => {
    const mark = manifest.marks.find((entry) => entry.step === step);
    if (!mark || !Number.isFinite(mark.seconds) || mark.seconds < 0 || videoTime(mark.seconds) >= duration) {
        throw new Error(`missing or invalid ${step} video mark`);
    }
    return videoTime(mark.seconds);
};
const chapters = [
    { step: 'create-market', label: '01  Create' },
    { step: 'deposit', label: '02  Deposit' },
    { step: 'open-curve-short', label: '03  Open' },
    { step: 'close-curve-short', label: '04  Close' },
    { step: 'scripted-liquidation', label: '05  Liquidate' },
    { step: 's1-settlement', label: '06  Settle' },
];
const chapterTimes = chapters.map(({ step }) => at(step));
if (chapterTimes.some((seconds, i) => i > 0 && seconds <= chapterTimes[i - 1])) {
    throw new Error('chapter marks must be in playback order');
}
at('outro');
const waits = manifest.waits
    .map((wait) => ({
        ...wait,
        capturedStart: wait.start,
        capturedEnd: wait.end,
        start: videoTime(wait.start),
        end: videoTime(wait.end),
    }))
    .sort((a, b) => a.start - b.start);
const segments: Segment[] = [];
let cursor = 0;
for (const wait of waits) {
    if (
        !['warmup', 'liquidation', 'settlement'].includes(wait.kind) ||
        !Number.isFinite(wait.start) ||
        !Number.isFinite(wait.end) ||
        wait.speed !== 8 ||
        wait.start < cursor ||
        wait.end <= wait.start ||
        wait.end > duration
    ) {
        throw new Error(`invalid or overlapping 8× wait: ${JSON.stringify(wait)}`);
    }
    if (wait.start > cursor) segments.push({ start: cursor, end: wait.start, speed: 1 });
    segments.push({ start: wait.start, end: wait.end, speed: wait.speed });
    cursor = wait.end;
}
if (cursor < duration) segments.push({ start: cursor, end: duration, speed: 1 });
const finalTime = (rawTime: number): number =>
    segments.reduce(
        (total, segment) => total + Math.max(0, Math.min(rawTime, segment.end) - segment.start) / segment.speed,
        0,
    );
const expectedDuration = finalTime(duration);
const cues = manifest.cues.map((cue, i) => {
    if (
        !cue.text?.trim() ||
        cue.text.includes('\n') ||
        !Number.isFinite(cue.seconds) ||
        cue.seconds < 0 ||
        videoTime(cue.seconds) >= duration ||
        (i > 0 && cue.seconds <= manifest.cues[i - 1].seconds)
    ) {
        throw new Error(`invalid or unordered cue ${i}`);
    }
    return { text: cue.text, start: finalTime(videoTime(cue.seconds)) };
});
const mappedWaits = waits.map((wait) => ({
    ...wait,
    outputStart: finalTime(wait.start),
    outputEnd: finalTime(wait.end),
}));

// ffmpeg reads text from files with expansion disabled, so punctuation in natural-language
// cues never becomes a filter expression. The output path is controlled by this script.
const font = '/usr/share/fonts/liberation/LiberationSans-Bold.ttf';
const regularFont = '/usr/share/fonts/liberation/LiberationSans-Regular.ttf';
const quotedPath = (path: string): string =>
    `'${path.replace(/\\/g, '\\\\').replace(/:/g, '\\:').replace(/'/g, "'\\''")}'`;
const textFile = (name: string, text: string): string => {
    const file = join(dir, `render-${name}.txt`);
    writeFileSync(file, text);
    return quotedPath(file);
};
const range = (start: number, end: number): string => `gte(t,${start.toFixed(4)})*lt(t,${end.toFixed(4)})`;
const drawText = (
    file: string,
    x: string | number,
    y: number,
    size: number,
    color: string,
    enable?: string,
    bold = true,
): string =>
    `drawtext=fontfile=${quotedPath(bold ? font : regularFont)}:textfile=${file}:expansion=none:fontcolor=${color}:fontsize=${size}:x=${x}:y=${y}${enable ? `:enable='${enable}'` : ''}`;
const parts = segments.map(
    ({ start, end, speed }, i) =>
        `[0:v]trim=start=${start.toFixed(4)}:end=${end.toFixed(4)},setpts=(PTS-STARTPTS)/${speed},fps=30[v${i}]`,
);
const desktopFooter = (): string[] => {
    const footer = [
        'pad=1440:1040:0:0:color=0x101620',
        'drawbox=x=0:y=900:w=1440:h=2:color=0xf0b90b:t=fill',
        'drawbox=x=24:y=949:w=1392:h=1:color=0x2a3443:t=fill',
    ];
    for (const [i, chapter] of chapters.entries()) {
        const x = 24 + i * 176;
        const label = textFile(`chapter-${i}`, chapter.label);
        const active = range(finalTime(chapterTimes[i]), finalTime(chapterTimes[i + 1] ?? duration));
        footer.push(
            `drawbox=x=${x}:y=909:w=164:h=32:color=0xf0b90b:t=fill:enable='${active}'`,
            drawText(label, x + 10, 916, 20, '0x96a3b5'),
            drawText(label, x + 10, 916, 20, '0x101620', active),
        );
    }
    const live = textFile('live', 'REAL TIME');
    const waitTitles = {
        warmup: '8×  MARKET WARM-UP',
        liquidation: '8×  LIQUIDATION WAIT',
        settlement: '8×  SETTLEMENT WAIT',
    };
    const anyWait = mappedWaits.map((wait) => range(wait.outputStart, wait.outputEnd)).join('+');
    footer.push(drawText(live, '1416-text_w', 917, 17, '0x74c9ad', `not(${anyWait})`));
    for (const [i, wait] of mappedWaits.entries()) {
        const enabled = range(wait.outputStart, wait.outputEnd);
        footer.push(
            `drawbox=x=1124:y=909:w=292:h=32:color=0x294054:t=fill:enable='${enabled}'`,
            drawText(textFile(`wait-${i}`, waitTitles[wait.kind]), '1270-text_w/2', 916, 19, '0x9cdbff', enabled),
        );
    }
    for (const [i, cue] of cues.entries()) {
        const next = cues[i + 1]?.start ?? expectedDuration + 0.2;
        // Cue text is a single line. Scale long instructions to fit the dedicated 1392 px area.
        const size = Math.min(27, Math.max(18, Math.floor(1392 / Math.max(1, cue.text.length * 0.56))));
        footer.push(drawText(textFile(`cue-${i}`, cue.text), 24, 967, size, '0xf4f7fb', range(cue.start, next)));
    }
    footer.push(
        drawText(
            textFile('version', `LOCAL VALIDATOR  ·  v${manifest.version}  ·  Not a deployment`),
            24,
            1012,
            17,
            '0x96a3b5',
            undefined,
            false,
        ),
        drawText(
            textFile('scenario', 'SCRIPTED EXITS  ·  Separate TEST-ONLY mock markets'),
            '1416-text_w',
            1012,
            17,
            '0xc7ab72',
            `gte(t,${finalTime(at('scripted-liquidation')).toFixed(4)})`,
            false,
        ),
    );
    return footer;
};
const phoneFooter = (): string[] => {
    const footer = [
        'scale=780:1560:flags=lanczos',
        'pad=1080:1920:150:64:color=0x101620',
        'drawbox=x=146:y=60:w=788:h=1568:color=0x364253:t=2',
        drawText(textFile('brand', 'short.win'), 150, 15, 34, '0xf0b90b'),
        drawText(textFile('format', 'PHONE DEMO'), '930-text_w', 23, 21, '0x96a3b5'),
        'drawbox=x=48:y=1828:w=984:h=1:color=0x2a3443:t=fill',
    ];
    const firstChapter = finalTime(chapterTimes[0]);
    footer.push(
        drawText(
            textFile('chapter-intro', 'A SHORT, FROM START TO FINISH'),
            48,
            1667,
            29,
            '0xf0b90b',
            range(0, firstChapter),
        ),
    );
    for (const [i, chapter] of chapters.entries()) {
        const x = 48 + i * 166;
        const start = finalTime(chapterTimes[i]);
        const end = finalTime(chapterTimes[i + 1] ?? duration);
        footer.push(
            `drawbox=x=${x}:y=1644:w=154:h=5:color=0x364253:t=fill`,
            `drawbox=x=${x}:y=1644:w=154:h=5:color=0xf0b90b:t=fill:enable='gte(t,${start.toFixed(4)})'`,
            drawText(textFile(`chapter-${i}`, chapter.label), 48, 1667, 32, '0xf0b90b', range(start, end)),
        );
    }
    const anyWait = mappedWaits.map((wait) => range(wait.outputStart, wait.outputEnd)).join('+');
    footer.push(drawText(textFile('live', 'REAL TIME'), '1032-text_w', 1673, 24, '0x74c9ad', `not(${anyWait})`));
    const waitTitles = {
        warmup: '8×  MARKET WARM-UP',
        liquidation: '8×  KEEPER WAIT',
        settlement: '8×  SETTLEMENT WAIT',
    };
    for (const [i, wait] of mappedWaits.entries()) {
        const enabled = range(wait.outputStart, wait.outputEnd);
        footer.push(
            `drawbox=x=644:y=1661:w=388:h=42:color=0x294054:t=fill:enable='${enabled}'`,
            drawText(textFile(`wait-${i}`, waitTitles[wait.kind]), '838-text_w/2', 1671, 25, '0x9cdbff', enabled),
        );
    }
    // Keep narration in two lines in the dedicated footer; no caption covers a control or
    // the phone's bottom navigation. Long sentences use a slightly smaller, still legible size.
    const wrap = (text: string, maxCharacters: number): string[] => {
        const lines: string[] = [];
        for (const word of text.split(/\s+/)) {
            const last = lines.length - 1;
            if (last >= 0 && lines[last].length + word.length + 1 <= maxCharacters) lines[last] += ` ${word}`;
            else lines.push(word);
        }
        return lines;
    };
    for (const [i, cue] of cues.entries()) {
        const next = cues[i + 1]?.start ?? expectedDuration + 0.2;
        let size = 40;
        let lines = wrap(cue.text, Math.floor(984 / (size * 0.56)));
        while (lines.length > 2 && size > 28) {
            size--;
            lines = wrap(cue.text, Math.floor(984 / (size * 0.56)));
        }
        if (lines.length > 2) throw new Error(`phone cue is too long for its two-line footer: ${cue.text}`);
        for (const [line, text] of lines.entries()) {
            footer.push(
                drawText(
                    textFile(`cue-${i}-${line}`, text),
                    48,
                    1722 + line * 49,
                    size,
                    '0xf4f7fb',
                    range(cue.start, next),
                ),
            );
        }
    }
    footer.push(
        drawText(
            textFile('version', `LOCAL VALIDATOR  ·  v${manifest.version}  ·  Not a deployment`),
            48,
            1846,
            27,
            '0x96a3b5',
            undefined,
            false,
        ),
        drawText(
            textFile('scenario', 'SCRIPTED EXITS  ·  Separate TEST-ONLY mock markets'),
            48,
            1886,
            24,
            '0xc7ab72',
            `gte(t,${finalTime(at('scripted-liquidation')).toFixed(4)})`,
            false,
        ),
    );
    return footer;
};
const footer = phone ? phoneFooter() : desktopFooter();
const filter = [
    ...parts,
    `${segments.map((_, i) => `[v${i}]`).join('')}concat=n=${segments.length}:v=1:a=0,${footer.join(',')}[out]`,
].join(';');
const suffix = phone ? '-phone' : '';
const mp4 = join(dir, `shortwin-local-validator-v${manifest.version}${suffix}.mp4`);
const gif = join(dir, `shortwin-local-validator-v${manifest.version}${suffix}.gif`);
const temporaryMp4 = join(dir, 'rendering.mp4');
const temporaryGif = join(dir, 'rendering.gif');
console.log(`[demo] encoding ${mp4} (${expectedDuration.toFixed(1)}s; ${waits.length} waits at 8×)`);
run('ffmpeg', [
    '-hide_banner',
    '-loglevel',
    'error',
    '-y',
    '-i',
    raw,
    '-filter_complex',
    filter,
    '-map',
    '[out]',
    '-c:v',
    'libx264',
    '-preset',
    'medium',
    '-crf',
    '18',
    '-pix_fmt',
    'yuv420p',
    '-movflags',
    '+faststart',
    '-an',
    temporaryMp4,
]);
const finalInfo = probe(temporaryMp4);
const finalDuration = Number(finalInfo.format.duration);
if (
    !(finalDuration >= 60 && finalDuration <= 240) ||
    Math.abs(finalDuration - expectedDuration) > 0.5 ||
    finalInfo.streams[0]?.width !== (phone ? 1080 : 1440) ||
    finalInfo.streams[0]?.height !== (phone ? 1920 : 1040)
) {
    throw new Error(`unexpected rendered video dimensions or duration: ${JSON.stringify(finalInfo)}`);
}
const gifStart = Math.max(0, Math.min(finalTime(at('open-curve-short')) + 2, finalDuration - 10));
console.log(`[demo] encoding ${gif}`);
run('ffmpeg', [
    '-hide_banner',
    '-loglevel',
    'error',
    '-y',
    '-ss',
    gifStart.toFixed(4),
    '-t',
    '10',
    '-i',
    temporaryMp4,
    '-filter_complex',
    `[0:v]fps=10,scale=${phone ? '540:960' : '960:-2'}:flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=sierra2_4a[v]`,
    '-map',
    '[v]',
    '-loop',
    '0',
    temporaryGif,
]);
const gifInfo = probe(temporaryGif);
if (
    Math.abs(Number(gifInfo.format.duration) - 10) > 0.05 ||
    gifInfo.streams[0]?.width !== (phone ? 540 : 960) ||
    (phone && gifInfo.streams[0]?.height !== 960)
) {
    throw new Error(`unexpected GIF duration or dimensions: ${JSON.stringify(gifInfo)}`);
}
run('ffmpeg', ['-hide_banner', '-loglevel', 'error', '-i', temporaryMp4, '-f', 'null', '-']);
renameSync(temporaryMp4, mp4);
renameSync(temporaryGif, gif);
writeFileSync(
    join(dir, 'render.json'),
    `${JSON.stringify(
        {
            version: manifest.version,
            ...(phone ? { variant: 'phone', viewport: manifest.viewport } : {}),
            rawDuration: duration,
            duration: finalDuration,
            dimensions: phone
                ? { width: 1080, height: 1920, viewportWidth: 780, viewportHeight: 1560, viewportX: 150, viewportY: 64 }
                : { width: 1440, height: 1040, viewportHeight: 900, footerHeight: 140 },
            fps: 30,
            pointer: manifest.pointer,
            timing: {
                method: 'Final title-card header transition; 144×4 grayscale samples at 30 fps, variance below 1',
                wallClockOffset,
                capturedOutro: outroMark,
                observedOutro,
                sourceManifestUnchanged: true,
            },
            waits: mappedWaits,
            chapters: chapters.map((chapter, i) => ({ ...chapter, seconds: finalTime(chapterTimes[i]) })),
            cues,
            gif: { start: gifStart, duration: 10, width: phone ? 540 : 960, height: gifInfo.streams[0].height },
            mp4,
            gifPath: gif,
        },
        null,
        2,
    )}\n`,
);
console.log(`[demo] ${finalDuration.toFixed(1)}s guided MP4 and 10s GIF ready`);

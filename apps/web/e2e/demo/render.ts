/** Render record.ts's one-take WebM as a captioned 2–3 minute MP4 and a 10-second GIF. */

import { spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { E2E_DIR } from '../env.ts';

const dir = join(E2E_DIR, 'demo');
const manifest = JSON.parse(readFileSync(join(dir, 'recording.json'), 'utf8')) as {
    version: string;
    failure: string | null;
    marks: { step: string; seconds: number }[];
    signatures: Record<string, string>;
};
if (manifest.failure) throw new Error(`recording failed: ${manifest.failure}`);
for (const step of ['deposit', 'open', 'liquidation', 's1']) {
    if (!manifest.signatures[step]) throw new Error(`missing ${step} signature`);
}
const at = (step: string): number => {
    const mark = manifest.marks.find((x) => x.step === step);
    if (!mark) throw new Error(`missing ${step} video mark`);
    return mark.seconds;
};
const run = (bin: string, args: string[]): string => {
    const r = spawnSync(bin, args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'] });
    if (r.status !== 0) throw new Error(`${bin} failed: ${r.status ?? r.signal}`);
    return r.stdout.trim();
};
const raw = join(dir, 'raw.webm');
const duration = Number(
    run('ffprobe', [
        '-v',
        'error',
        '-show_entries',
        'format=duration',
        '-of',
        'default=noprint_wrappers=1:nokey=1',
        raw,
    ]),
);
if (!Number.isFinite(duration)) throw new Error('ffprobe did not report a duration');

// Keep every browser action at real speed. Only the static 50-second market warm-up and the
// keeper's repeated settlement waits run at 2x. Both accelerated parts are labelled on screen.
const warmStart = at('deposit') + 11.5;
const warmEnd = at('open-curve-short') - 2;
const keeperStart = at('s1-settlement') + 8;
const keeperEnd = at('outro') - 6.5;
if (
    !(0 < warmStart && warmStart < warmEnd && warmEnd < keeperStart && keeperStart < keeperEnd && keeperEnd < duration)
) {
    throw new Error('video marks cannot define the two wait intervals');
}

const caption = join(dir, 'caption.txt');
const warmLabel = join(dir, 'warmup.txt');
const keeperLabel = join(dir, 'keeper.txt');
const scenarioLabel = join(dir, 'scenario.txt');
const outroTitle = join(dir, 'outro-title.txt');
const outroDetail = join(dir, 'outro-detail.txt');
writeFileSync(caption, `local validator, v${manifest.version}, not a deployment.`);
writeFileSync(warmLabel, 'Market warm-up · wait shown at 2× speed');
writeFileSync(keeperLabel, 'Keeper settlement rounds · wait shown at 2× speed');
writeFileSync(scenarioLabel, 'Scripted exits · separate TEST-ONLY mock markets');
writeFileSync(outroTitle, 'Six local-validator actions');
writeFileSync(outroDetail, 'Curve close, then liquidation and S1 settlement on separate test markets.');
const font = '/usr/share/fonts/liberation/LiberationSans-Bold.ttf';
const label = (file: string) =>
    `drawtext=fontfile=${font}:textfile=${file}:fontcolor=white:fontsize=25:box=1:boxcolor=black@0.8:boxborderw=12:x=w-text_w-35:y=58`;
const segments: [number, number, number, string?][] = [
    [0, warmStart, 1],
    [warmStart, warmEnd, 2, warmLabel],
    [warmEnd, keeperStart, 1],
    [keeperStart, keeperEnd, 2, keeperLabel],
    [keeperEnd, duration, 1],
];
const parts = segments.map(
    ([start, end, speed, text], i) =>
        `[0:v]trim=start=${start.toFixed(3)}:end=${end.toFixed(3)},setpts=(PTS-STARTPTS)/${speed},fps=30${text ? `,${label(text)}` : ''}[v${i}]`,
);
// The raw one-take outro used singular wording. Replace only that title card in the shareable
// cut so viewers know the two scripted exits took place on separate mock markets.
const outroWithinLast = at('outro') - keeperEnd - 0.2;
const outroEnable = `enable='gte(t,${outroWithinLast.toFixed(3)})'`;
parts[4] = parts[4]?.replace(
    '[v4]',
    `,drawbox=x=0:y=0:w=iw:h=ih:color=0x0b0e11:t=fill:${outroEnable}` +
        `,drawtext=fontfile=${font}:textfile=${outroTitle}:fontcolor=0xf0b90b:fontsize=46:x=(w-text_w)/2:y=h/2-42:${outroEnable}` +
        `,drawtext=fontfile=${font}:textfile=${outroDetail}:fontcolor=white:fontsize=23:x=(w-text_w)/2:y=h/2+30:${outroEnable}[v4]`,
);
// Dialogs reach near the bottom of the viewport; the caption fits in the status strip.
const versionCaption = `drawtext=fontfile=${font}:textfile=${caption}:fontcolor=white:fontsize=21:box=1:boxcolor=black@0.9:boxborderw=4:x=(w-text_w)/2:y=h-text_h-7`;
const liquidationInFinal = at('scripted-liquidation') - (warmEnd - warmStart) / 2;
const outroInFinal = at('outro') - (warmEnd - warmStart) / 2 - (keeperEnd - keeperStart) / 2;
const scenarioCaption = `drawtext=fontfile=${font}:textfile=${scenarioLabel}:fontcolor=white:fontsize=24:box=1:boxcolor=black@0.8:boxborderw=10:x=24:y=430:enable='between(t,${liquidationInFinal.toFixed(3)},${outroInFinal.toFixed(3)})'`;
const filter = [
    ...parts,
    `${segments.map((_, i) => `[v${i}]`).join('')}concat=n=${segments.length}:v=1:a=0,${scenarioCaption},${versionCaption}[out]`,
].join(';');
const mp4 = join(dir, `shortwin-local-validator-v${manifest.version}.mp4`);
const gif = join(dir, `shortwin-local-validator-v${manifest.version}.gif`);
console.log(`[demo] encoding ${mp4}`);
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
    mp4,
]);
const openInFinal = at('open-curve-short') - (warmEnd - warmStart) / 2;
const gifStart = Math.max(0, openInFinal - 1);
console.log(`[demo] encoding ${gif}`);
run('ffmpeg', [
    '-hide_banner',
    '-loglevel',
    'error',
    '-y',
    '-ss',
    gifStart.toFixed(3),
    '-t',
    '10',
    '-i',
    mp4,
    '-filter_complex',
    '[0:v]fps=10,scale=960:-2:flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=sierra2_4a[v]',
    '-map',
    '[v]',
    '-loop',
    '0',
    gif,
]);
const finalDuration = Number(
    run('ffprobe', [
        '-v',
        'error',
        '-show_entries',
        'format=duration',
        '-of',
        'default=noprint_wrappers=1:nokey=1',
        mp4,
    ]),
);
if (!(finalDuration >= 120 && finalDuration <= 180)) {
    throw new Error(`rendered video is ${finalDuration}s; expected 2–3 minutes`);
}
console.log(`[demo] ${finalDuration.toFixed(1)}s MP4 and 10s GIF ready`);

import fs from 'node:fs';
import path from 'node:path';
import {spawnSync} from 'node:child_process';

const destination = process.env.SHORTWIN_VIDEO_OUTPUT || '/home/todor_dev/Videos/shortwin-2026-11-02';
const films = [
  {name: '01-lend', width: 1080, height: 1920, seconds: 24},
  {name: '02-short', width: 1080, height: 1920, seconds: 28},
  {name: '03-manage-close', width: 1080, height: 1920, seconds: 28},
  {name: '04-overview', width: 1920, height: 1080, seconds: 45},
];
const run = (binary, args) => {
  const result = spawnSync(binary, args, {encoding: 'utf8'});
  if (result.status !== 0) throw new Error(`${binary} failed: ${result.stderr || result.stdout}`);
  return result.stdout;
};
const assert = (test, message) => {if (!test) throw new Error(message);};
const verified = [];
for (const film of films) {
  const file = path.join(destination, `${film.name}.mp4`);
  const metadata = JSON.parse(run('ffprobe', ['-v', 'error', '-show_streams', '-show_format', '-of', 'json', file]));
  const video = metadata.streams.find((stream) => stream.codec_type === 'video');
  const audio = metadata.streams.find((stream) => stream.codec_type === 'audio');
  assert(video?.codec_name === 'h264', `${film.name}: expected H.264`);
  assert(video.width === film.width && video.height === film.height, `${film.name}: incorrect dimensions`);
  assert(video.r_frame_rate === '30/1', `${film.name}: incorrect frame rate`);
  assert(Number(video.nb_frames) === film.seconds * 30, `${film.name}: incorrect frame count`);
  assert(video.pix_fmt === 'yuv420p', `${film.name}: incorrect pixel format`);
  assert(Math.abs(Number(metadata.format.duration) - film.seconds) < .05, `${film.name}: incorrect duration`);
  assert(audio?.codec_name === 'aac' && audio.channels === 2, `${film.name}: expected stereo AAC`);
  assert(Math.abs(Number(audio.duration) - film.seconds) < .05, `${film.name}: audio does not cover the full film`);
  run('ffmpeg', ['-v', 'error', '-xerror', '-i', file, '-f', 'null', '-']);
  const poster = path.join(destination, `${film.name}-poster.jpg`);
  const posterMetadata = JSON.parse(run('ffprobe', ['-v', 'error', '-show_streams', '-of', 'json', poster]));
  assert(posterMetadata.streams[0].width === film.width && posterMetadata.streams[0].height === film.height, `${film.name}: poster dimensions differ`);
  verified.push({...film, videoCodec: video.codec_name, pixelFormat: video.pix_fmt, fps: 30, audioCodec: audio.codec_name, audioChannels: audio.channels, frames: Number(video.nb_frames), bytes: fs.statSync(file).size, fullDecode: 'passed'});
  console.log(`Verified ${film.name}: ${film.seconds}s, ${film.width}×${film.height}, 30fps, H.264/AAC, full decode passed.`);
}
const archive = path.join(destination, 'shortwin-editable-project.zip');
run('unzip', ['-t', archive]);
const entries = run('unzip', ['-Z1', archive]).split('\n');
assert(entries.includes('captions.txt'), 'Editable archive: social captions missing');
assert(entries.includes('public/audio/original-score.m4a'), 'Editable archive: original score missing');
assert(!entries.some((entry) => entry.includes('forest') || entry.includes('lime')), 'Editable archive: stale brand asset remains');
const archivedFilm = run('unzip', ['-p', archive, 'src/Film.tsx']);
assert(archivedFilm.includes('M1 4L4.75 20L8.44 20L12 13.53L12.4 14.25L12.4 7.2L12 6.47L7.14 15.3L4.49 4Z') && archivedFilm.includes('M12 13.53L15.56 20L19.25 20L23 4L19.51 4L16.86 15.3L12 6.47Z'), 'Editable archive: original W paths missing');
assert(archivedFilm.includes("const white = '#e8ecf1'") && archivedFilm.includes("const orange = '#f0b90b'"), 'Editable archive: original brand colors missing');
const report = {verifiedAt: new Date().toISOString(), films: verified, editableArchive: 'integrity passed; original W and palette, score, and captions present; stale brand assets absent', disclosure: 'Local historical fork, test funds, synthetic price history. Captures include separate successful local workflow takes.'};
fs.writeFileSync(path.join(destination, 'verification.json'), JSON.stringify(report, null, 2) + '\n');
console.log('Editable project archive integrity passed. Wrote verification.json.');

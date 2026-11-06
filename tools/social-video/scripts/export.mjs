import fs from 'node:fs';
import path from 'node:path';
import {spawnSync} from 'node:child_process';
import {projectRoot as root, videoOutput as destination} from './paths.mjs';

const assets = ['workspace', 'lend-form', 'lend-review', 'lend-success', 'short-form', 'short-review', 'short-success', 'position', 'margin', 'close-review', 'close-success'];
const missing = assets.filter((name) => !fs.existsSync(path.join(root, 'public/captures', `${name}.jpg`)));
if (missing.length) throw new Error(`Final export blocked: real captures still missing: ${missing.join(', ')}`);
const previewOnly = process.argv.includes('--previews');
// Use this project's locked CLI; npx may otherwise fetch a different version
// when dependencies are missing, making the exported film irreproducible.
const cli = path.join(root, 'node_modules/@remotion/cli/remotion-cli.js');
if (!fs.existsSync(cli)) throw new Error('Install the locked video dependencies with npm ci before exporting.');
const run = (...args) => {
  const result = spawnSync(process.execPath, [cli, ...args], {cwd: root, stdio: 'inherit'});
  if (result.error) throw new Error(`Could not start the pinned Remotion CLI: ${result.error.message}`);
  if (result.status !== 0) process.exit(result.status || 1);
};
fs.mkdirSync(destination, {recursive: true});
const edits = [
  ['Lend', '01-lend', 240],
  ['Short', '02-short', 240],
  ['ManageClose', '03-manage-close', 210],
  ['Overview', '04-overview', 270],
];
if (previewOnly) {
  for (const [composition, basename, frame] of edits) {
    run('still', composition, path.join(destination, `${basename}-preview.jpg`), `--frame=${frame}`, '--image-format=jpeg', '--jpeg-quality=95');
  }
  process.exit(0);
}
for (const [composition, basename, frame] of edits) {
  run('render', composition, path.join(destination, `${basename}.mp4`), '--codec=h264', '--crf=18', '--pixel-format=yuv420p', '--audio-codec=aac', '--audio-bitrate=192k', '--concurrency=4');
  run('still', composition, path.join(destination, `${basename}-poster.jpg`), `--frame=${frame}`, '--image-format=jpeg', '--jpeg-quality=95');
}
for (const document of ['README.md', 'captions.txt']) {
  fs.copyFileSync(path.join(root, document), path.join(destination, document));
}
const archive = spawnSync('zip', ['-qr', '-FS', path.join(destination, 'shortwin-editable-project.zip'), 'src', 'public', 'scripts', 'package.json', 'package-lock.json', 'remotion.config.ts', 'tsconfig.json', 'eslint.config.mjs', '.gitignore', '.prettierrc', 'README.md', 'captions.txt', '-x', 'public/audio/*.wav'], {cwd: root, stdio: 'inherit'});
if (archive.error) throw new Error(`Could not create the editable project archive: ${archive.error.message}`);
if (archive.status !== 0) process.exit(archive.status || 1);
console.log(`Exports saved to ${destination}`);

import {copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync} from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import assert from 'node:assert/strict';
import {runInNewContext} from 'node:vm';
import {videoOutput as mediaDirectory} from '../tools/social-video/scripts/paths.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const webDirectory = path.join(root, 'web');
const destination = path.join(webDirectory, 'dist');
const films = ['01-lend', '02-short', '03-manage-close', '04-overview'];
const media = films.flatMap((name) => [`${name}.mp4`, `${name}-poster.jpg`]);
for (const filename of media) {
  const source = path.join(mediaDirectory, filename);
  assert(existsSync(source) && statSync(source).size > 0, `Missing showcase media: ${source}. Render tools/social-video first or set SHORTWIN_VIDEO_OUTPUT.`);
}
const result = spawnSync('pnpm', ['--dir', webDirectory, 'build', '--manifest'], {
  cwd: root,
  env: {...process.env, VITE_PUBLIC_PREVIEW: 'true', VITE_ENABLE_LOCAL_DEMO: 'false'},
  stdio: 'inherit',
});
if (result.status !== 0) process.exit(result.status || 1);

const manifest = JSON.parse(readFileSync(path.join(destination, '.vite/manifest.json'), 'utf8'));
const manifestEntries = Object.keys(manifest);
assert.deepEqual(manifestEntries.sort(), ['index.html', 'src/public.ts'], 'Public build must contain only the entry and showcase; operational app/SDK chunks are forbidden.');
assert(manifest['src/public.ts'].isDynamicEntry, 'The public showcase must be selected by the entry.');
const walk = (directory) => readdirSync(directory, {withFileTypes: true}).flatMap((entry) => {
  const filename = path.join(directory, entry.name);
  return entry.isDirectory() ? walk(filename) : [filename];
});
const builtFiles = walk(destination);
assert(!builtFiles.some((filename) => filename.endsWith('.map')), 'Public source maps must not be deployed.');
const javascript = builtFiles.filter((filename) => filename.endsWith('.js'));
assert.equal(javascript.length, 2, 'Public build must not include app/SDK JavaScript chunks.');
const source = javascript.map((filename) => readFileSync(filename, 'utf8')).join('\n');
assert(source.length < 80_000, 'Unexpectedly large public bundle; inspect for operational dependencies.');
assert(!/(?:@solana|createSolanaRpc|createNoopSigner|signTransaction|connectWallet|getGenesisHash|solana:devnet|127\.0\.0\.1:8899|127\.0\.0\.1:8787|localhost)/i.test(source), 'Public bundle contains operational SDK, wallet, or local RPC/service code.');
assert(!/id=["'](?:connect|wallet|mint|sign)["']/.test(source), 'Public page contains operational wallet controls.');
const rendered = {innerHTML: ''};
runInNewContext(readFileSync(path.join(destination, manifest['src/public.ts'].file), 'utf8'), {
  document: {querySelector: (selector) => { assert.equal(selector, '#app'); return rendered; }},
}, {timeout: 1000});
assert.equal((rendered.innerHTML.match(/<video /g) || []).length, 4, 'Public page must render four walkthroughs.');
for (const name of films) {
  assert(rendered.innerHTML.includes(`src="/videos/${name}.mp4"`), `Public page is missing video ${name}.`);
  assert(rendered.innerHTML.includes(`poster="/videos/${name}-poster.jpg"`), `Public page is missing poster ${name}.`);
}
assert.equal((rendered.innerHTML.match(/controls playsinline preload="none"/g) || []).length, 4, 'Public videos must use native controls and avoid preloading media.');
assert(rendered.innerHTML.includes('synthetic price history'), 'Public demo disclosure is missing.');
const sourceRef = (process.env.VITE_SOURCE_REF?.trim() || 'main').split('/').map(encodeURIComponent).join('/');
for (const document of ['scripts/local-fork/README.md', 'docs/SPEC.md', 'docs/SCOPE.md', 'SECURITY.md']) {
  assert(rendered.innerHTML.includes(`https://github.com/TodorBonchev/short.win/blob/${sourceRef}/${document}`), `Public documentation link does not match source ref: ${document}.`);
}
mkdirSync(path.join(destination, 'videos'), {recursive: true});
for (const filename of media) copyFileSync(path.join(mediaDirectory, filename), path.join(destination, 'videos', filename));
// Build metadata is useful for this check, but unnecessary in the deployed site.
rmSync(path.join(destination, '.vite'), {recursive: true});
console.log(`Public showcase verified: ${javascript.length} JavaScript files, ${(source.length / 1024).toFixed(1)} KiB, four videos and four posters. No wallet/SDK chunks or source maps.`);

// Copies the Anchor IDL of the program build into clients/js/idl/shortwin.json, the input of
// `pnpm run generate` (codama.json). Source: the first argument, else $SHORTWIN_IDL, else
// <repo>/target/idl/shortwin.json (the output of `anchor build --tools-version v1.57 --arch v3`).
import { copyFileSync, existsSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, '../../..');
const source = resolve(process.argv[2] ?? process.env.SHORTWIN_IDL ?? resolve(repo, 'target/idl/shortwin.json'));
const target = resolve(here, '../idl/shortwin.json');

if (!existsSync(source)) {
    console.error(`no IDL at ${source}: run \`anchor build --tools-version v1.57 --arch v3\` first (or pass a path / set SHORTWIN_IDL)`);
    process.exit(1);
}
copyFileSync(source, target);
console.log(`${source} -> ${target}`);

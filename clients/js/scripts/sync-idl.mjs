// Copies the Anchor IDLs of the program build into clients/js/idl/:
// - shortwin.json, the input of `pnpm run generate` (codama.json). Source: the first argument,
//   else $SHORTWIN_IDL, else <repo>/target/idl/shortwin.json (the output of
//   `anchor build --tools-version v1.57 --arch v3`).
// - complete_curve.json, the straddle helper's IDL (M5; ARCH §5.4), committed for clients and
//   the audit package (docs/AUDIT.md); nothing is generated from it. Source: $COMPLETE_CURVE_IDL,
//   else complete_curve.json next to the shortwin source.
import { copyFileSync, existsSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, '../../..');
const source = resolve(process.argv[2] ?? process.env.SHORTWIN_IDL ?? resolve(repo, 'target/idl/shortwin.json'));
const helperSource = resolve(process.env.COMPLETE_CURVE_IDL ?? resolve(dirname(source), 'complete_curve.json'));

const copies = [
    [source, 'shortwin.json'],
    [helperSource, 'complete_curve.json'],
];
const missing = copies.map(([from]) => from).filter((from) => !existsSync(from));
if (missing.length > 0) {
    console.error(
        `no IDL at ${missing.join(', ')}: run \`anchor build --tools-version v1.57 --arch v3\` first (or pass a path / set SHORTWIN_IDL, COMPLETE_CURVE_IDL)`,
    );
    process.exit(1);
}
for (const [from, name] of copies) {
    const target = resolve(here, '../idl', name);
    copyFileSync(from, target);
    console.log(`${from} -> ${target}`);
}

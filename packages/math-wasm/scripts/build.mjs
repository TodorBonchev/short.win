#!/usr/bin/env node
import { execFileSync, spawnSync } from 'node:child_process';
// Builds packages/math-wasm/pkg from crates/shortwin-math-wasm (ARCHITECTURE §1.2, §7.3;
// FRONTEND.md §6.4):
//
//   cargo build -p shortwin-math-wasm --target wasm32-unknown-unknown --release --locked
//   wasm-bindgen --target web --out-dir packages/math-wasm/pkg <the .wasm>
//
// pkg/ is generated and not committed (the .wasm embeds absolute registry paths, so it is not
// byte-reproducible across machines). The build is skipped when pkg/ holds the output of the
// same inputs: a SHA-256 over the three crates' sources and manifests (shortwin-math,
// shortwin-venue-state for the mint filter, shortwin-math-wasm) and the root Cargo.lock is
// written to pkg/.inputs-sha256 and compared on the next run (`--force` rebuilds anyway).
//
// wasm-bindgen must be the CLI matching the crate's exact pin (=0.2.129). It is looked up in
// $WASM_BINDGEN, then on PATH, then in ~/.cargo/bin. Install it with
//   cargo install wasm-bindgen-cli --version =0.2.129 --locked
// cargo is looked up in $CARGO, then on PATH, then in ~/.cargo/bin; CARGO_TARGET_DIR is honoured
// (the default is <repo>/target, on disk).
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const WASM_BINDGEN_VERSION = '0.2.129';
const here = dirname(fileURLToPath(import.meta.url));
const pkgRoot = resolve(here, '..');
const repo = resolve(pkgRoot, '..', '..');
const outDir = join(pkgRoot, 'pkg');
const stampFile = join(outDir, '.inputs-sha256');
const force = process.argv.includes('--force');

function listFiles(dir) {
    const out = [];
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
        const p = join(dir, entry.name);
        if (entry.isDirectory()) out.push(...listFiles(p));
        else out.push(p);
    }
    return out;
}

function inputsHash() {
    const files = [
        ...listFiles(join(repo, 'crates', 'shortwin-math', 'src')),
        ...listFiles(join(repo, 'crates', 'shortwin-venue-state', 'src')),
        ...listFiles(join(repo, 'crates', 'shortwin-math-wasm', 'src')),
        join(repo, 'crates', 'shortwin-math', 'Cargo.toml'),
        join(repo, 'crates', 'shortwin-venue-state', 'Cargo.toml'),
        join(repo, 'crates', 'shortwin-math-wasm', 'Cargo.toml'),
        join(repo, 'Cargo.lock'),
    ].sort();
    const h = createHash('sha256');
    for (const f of files) {
        h.update(f.slice(repo.length));
        h.update('\0');
        h.update(readFileSync(f));
        h.update('\0');
    }
    return h.digest('hex');
}

function findCargo() {
    for (const c of [process.env.CARGO, 'cargo', join(homedir(), '.cargo', 'bin', 'cargo')]) {
        if (c && spawnSync(c, ['--version'], { encoding: 'utf8', cwd: repo }).status === 0) return c;
    }
    return null;
}

function findWasmBindgen() {
    const candidates = [];
    if (process.env.WASM_BINDGEN) candidates.push(process.env.WASM_BINDGEN);
    candidates.push('wasm-bindgen');
    candidates.push(join(homedir(), '.cargo', 'bin', 'wasm-bindgen'));
    for (const c of candidates) {
        const r = spawnSync(c, ['--version'], { encoding: 'utf8' });
        if (r.status === 0) {
            const version = r.stdout.trim().split(/\s+/)[1];
            if (version === WASM_BINDGEN_VERSION) return c;
            console.error(`math-wasm: ${c} is wasm-bindgen ${version}; the crate pins =${WASM_BINDGEN_VERSION}`);
        }
    }
    return null;
}

const want = inputsHash();
const expected = ['shortwin_math_wasm.js', 'shortwin_math_wasm.d.ts', 'shortwin_math_wasm_bg.wasm'];
const have = existsSync(stampFile) ? readFileSync(stampFile, 'utf8').trim() : null;
if (!force && have === want && expected.every((f) => existsSync(join(outDir, f)))) {
    console.log('math-wasm: pkg/ is up to date');
    process.exit(0);
}

const bindgen = findWasmBindgen();
if (!bindgen) {
    console.error(
        [
            `math-wasm: pkg/ is missing or stale and no wasm-bindgen ${WASM_BINDGEN_VERSION} was found.`,
            `Install it:  cargo install wasm-bindgen-cli --version =${WASM_BINDGEN_VERSION} --locked`,
            'or point $WASM_BINDGEN at a matching binary.',
        ].join('\n'),
    );
    process.exit(1);
}

const cargo = findCargo();
if (!cargo) {
    console.error('math-wasm: pkg/ is missing or stale and cargo was not found ($CARGO, PATH, ~/.cargo/bin).');
    process.exit(1);
}
console.log('math-wasm: cargo build -p shortwin-math-wasm --target wasm32-unknown-unknown --release');
execFileSync(
    cargo,
    ['build', '-p', 'shortwin-math-wasm', '--target', 'wasm32-unknown-unknown', '--release', '--locked'],
    { cwd: repo, stdio: 'inherit' },
);
const targetDir = process.env.CARGO_TARGET_DIR ? resolve(repo, process.env.CARGO_TARGET_DIR) : join(repo, 'target');
const wasm = join(targetDir, 'wasm32-unknown-unknown', 'release', 'shortwin_math_wasm.wasm');
if (!existsSync(wasm) || statSync(wasm).size === 0) {
    console.error(`math-wasm: ${wasm} was not produced`);
    process.exit(1);
}
rmSync(outDir, { recursive: true, force: true });
mkdirSync(outDir, { recursive: true });
console.log(`math-wasm: ${bindgen} --target web --out-dir pkg`);
execFileSync(bindgen, ['--target', 'web', '--out-dir', outDir, wasm], { stdio: 'inherit' });
writeFileSync(stampFile, `${want}\n`);
console.log('math-wasm: done');

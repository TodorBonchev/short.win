/**
 * The drift test of the Mobile Wallet Adapter (FRONTEND.md §5.4 (6), §9.3, §9.6): the installed
 * `@solana-mobile/wallet-standard-mobile` 0.7.0 and its protocol 3.0.0 are byte for byte the files
 * the design reviewed, their in-page dependencies are the versions recorded at install, and the
 * library facts the app relies on (the spinner's CSP hashes, the host shapes the observer removes,
 * the only interpolated `innerHTML`, the default account features, the calls that never settle)
 * still hold. A package or lockfile change fails here until someone re-reads the library.
 */
import { createHash } from 'node:crypto';
import { existsSync, readFileSync, realpathSync } from 'node:fs';
import { basename, dirname, join, resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import { MWA_SPINNER_ATTR_HASH, MWA_SPINNER_STYLE_HASH } from '../config/csp.ts';
import { REPO } from '../test/repo.ts';

const WEB = resolve(REPO, 'apps', 'web');
const WSM = realpathSync(join(WEB, 'node_modules', '@solana-mobile', 'wallet-standard-mobile'));

/** A package's `node_modules` root in pnpm's store layout (its dependencies are linked there). */
function modulesOf(pkgDir: string): string {
    const parent = dirname(pkgDir);
    return basename(parent).startsWith('@') ? dirname(parent) : parent;
}
/** Where `name` resolves from the package at `pkgDir` (through pnpm's links). */
function depOf(pkgDir: string, name: string): string {
    return realpathSync(join(modulesOf(pkgDir), name));
}
function manifest(pkgDir: string): { name: string; version: string; dependencies?: Record<string, string> } {
    return JSON.parse(readFileSync(join(pkgDir, 'package.json'), 'utf8'));
}
const sha256Hex = (bytes: Buffer | string) => createHash('sha256').update(bytes).digest('hex');
const sha256B64 = (text: string) => createHash('sha256').update(text, 'utf8').digest('base64');

const PROTOCOL = depOf(WSM, '@solana-mobile/mobile-wallet-adapter-protocol');
const WSM_JS = readFileSync(join(WSM, 'lib', 'esm', 'index.browser.js'), 'utf8');

/**
 * Every package the library can bring into the page (its whole dependency closure as linked, not
 * descending into Kit: the app's own `@solana/kit` 8.3.0, pinned by its package.json and the
 * workspace override), recorded at install on 2026-10-13. Every one of them is at least 14 days
 * old except the two `@solana-mobile` packages (`minimumReleaseAge`, pnpm-workspace.yaml).
 */
const IN_PAGE = [
    '@noble/curves@1.9.7',
    '@noble/curves@2.4.0',
    '@noble/hashes@1.8.0',
    '@noble/hashes@2.4.0',
    '@solana-mobile/mobile-wallet-adapter-protocol@3.0.0',
    '@solana/kit@8.3.0',
    '@solana/wallet-standard-chains@1.1.2',
    '@solana/wallet-standard-features@1.5.0',
    '@solana/wallet-standard-util@1.1.4',
    '@wallet-standard/app@1.1.1',
    '@wallet-standard/base@1.1.1',
    '@wallet-standard/core@1.1.2',
    '@wallet-standard/errors@0.1.2',
    '@wallet-standard/features@1.1.1',
    '@wallet-standard/wallet@1.1.1',
    'ansi-regex@5.0.1',
    'ansi-styles@4.3.0',
    'camelcase@5.3.1',
    'chalk@5.6.2',
    'cliui@6.0.0',
    'color-convert@2.0.1',
    'color-name@1.1.4',
    'commander@13.1.0',
    'decamelize@1.2.0',
    'dijkstrajs@1.0.3',
    'emoji-regex@8.0.0',
    'find-up@4.1.0',
    'get-caller-file@2.0.5',
    'is-fullwidth-code-point@3.0.0',
    'locate-path@5.0.0',
    'p-limit@2.3.0',
    'p-locate@4.1.0',
    'p-try@2.2.0',
    'path-exists@4.0.0',
    'pngjs@5.0.0',
    'qrcode@1.5.4',
    'require-directory@2.1.1',
    'require-main-filename@2.0.0',
    'set-blocking@2.0.0',
    'string-width@4.2.3',
    'strip-ansi@6.0.1',
    'tslib@2.8.1',
    'which-module@2.0.1',
    'wrap-ansi@6.2.0',
    'y18n@4.0.3',
    'yargs-parser@18.1.3',
    'yargs@15.4.1',
];

/** The closure of `pkgDir`'s dependencies as `name@version`, read through `realpath` from each importer. */
function closure(pkgDir: string, out = new Set<string>(), seen = new Set<string>()): Set<string> {
    if (seen.has(pkgDir)) return out;
    seen.add(pkgDir);
    for (const name of Object.keys(manifest(pkgDir).dependencies ?? {})) {
        const link = join(modulesOf(pkgDir), name);
        expect(existsSync(link), `${manifest(pkgDir).name} → ${name}`).toBe(true);
        const dir = realpathSync(link);
        const m = manifest(dir);
        out.add(`${m.name}@${m.version}`);
        if (m.name !== '@solana/kit') closure(dir, out, seen);
    }
    return out;
}

describe('the Mobile Wallet Adapter package (drift test)', () => {
    it('is wallet-standard-mobile 0.7.0 over protocol 3.0.0, the files the design reviewed', () => {
        expect(manifest(WSM)).toMatchObject({ name: '@solana-mobile/wallet-standard-mobile', version: '0.7.0' });
        expect(manifest(PROTOCOL)).toMatchObject({
            name: '@solana-mobile/mobile-wallet-adapter-protocol',
            version: '3.0.0',
        });
        expect(sha256Hex(readFileSync(join(WSM, 'lib', 'esm', 'index.browser.js')))).toBe(
            'fd03abc589a4e7effee510e810d2ab6a3f9e6e7122683076a1a767e358cabd62',
        );
        const files: [string, string][] = [
            ['index.browser.js', 'f78ec90af6a219323aa4de5391113d65cffe7f6ae53338a1cbfad69b0ff832d8'],
            ['chunks/encoding.browser.js', 'f4e30b6aaed378288e433630b040e2dc02b7674b7f76a146fefa99201947cad6'],
            ['encoding.browser.js', '4a3ee2fc05e2a13dbeac30027f8bc3a896362eceee80f0112bd0ff43945cb8f9'],
        ];
        for (const [file, hash] of files) {
            expect(sha256Hex(readFileSync(join(PROTOCOL, 'lib', 'esm', file))), file).toBe(hash);
        }
    });

    it('brings exactly the in-page dependencies recorded at install', () => {
        expect([...closure(WSM)].sort()).toEqual(IN_PAGE);
        // The protocol's own `@solana/kit: ^8.3.0` resolves to the app's Kit (the scoped override).
        expect(manifest(depOf(PROTOCOL, '@solana/kit')).version).toBe('8.3.0');
        expect(manifest(realpathSync(join(WEB, 'node_modules', '@solana', 'kit'))).version).toBe('8.3.0');
    });

    it('has the spinner whose <style> text and style attribute csp.ts hashes', () => {
        const open = 'const css$1 = `';
        const at = WSM_JS.indexOf(open);
        expect(at).toBeGreaterThan(0);
        const css = WSM_JS.slice(at + open.length, WSM_JS.indexOf('`;', at));
        // A plain template literal: the bundled text is the same string.
        expect(css).not.toContain('\\');
        expect(css).not.toContain('${');
        expect(css).toContain('.mobile-wallet-adapter-embedded-loading-indicator');
        expect(css.length).toBe(2362);
        expect(`'sha256-${sha256B64(css)}'`).toBe(MWA_SPINNER_STYLE_HASH);
        // The spinner's markup (`modalHtml`, the one before `css$1`) has exactly one style attribute.
        const html = WSM_JS.slice(WSM_JS.lastIndexOf('const modalHtml = `', at), at);
        const attrs = [...html.matchAll(/ style="([^"]*)"/g)].map((m) => m[1]);
        const attr = 'position: absolute; width: 100%; height: 100%;';
        expect(attrs).toEqual([attr]);
        expect(`'sha256-${sha256B64(attr)}'`).toBe(MWA_SPINNER_ATTR_HASH);
    });

    it('writes an interpolated innerHTML only in ErrorModal.populateError (never built: the app passes onWalletNotFound)', () => {
        const interpolated = [...WSM_JS.matchAll(/innerHTML = `[^`]*\$\{/g)];
        expect(interpolated).toHaveLength(1);
        const from = WSM_JS.indexOf('populateError(error) {');
        const to = WSM_JS.indexOf('const ErrorDialogHtml$3', from);
        expect(from).toBeGreaterThan(0);
        const index = interpolated[0]?.index ?? -1;
        expect(index > from && index < to).toBe(true);
        // The default handler is the only caller of initWithError; registerMwa uses it only by default.
        expect(WSM_JS).toContain('onWalletNotFound = createDefaultWalletNotFoundHandler()');
    });

    it('names its wallet "Mobile Wallet Adapter"', () => {
        expect(/const SolanaMobileWalletAdapterWalletName = "([^"]+)";/.exec(WSM_JS)?.[1]).toBe(
            'Mobile Wallet Adapter',
        );
    });

    it('appends the two host shapes the app removes at insertion (removeLibraryUi)', () => {
        const count = (s: string) => WSM_JS.split(s).length - 1;
        // The spinner's host carries the custom property, set through the CSSOM before the append.
        expect(count('host.style.setProperty("--spinner-color"')).toBe(1);
        // Every embedded modal's host holds the Google Fonts links in its light DOM.
        expect(count('host.innerHTML = fonts')).toBe(1);
        const fonts = /const fonts = `([^`]*)`;/.exec(WSM_JS)?.[1] ?? '';
        expect(fonts).toContain('<link href="https://fonts.googleapis.com/');
        // Both hosts are appended straight to <body> (a childList observer on body sees them).
        expect(count('document.body.appendChild(host)')).toBe(2);
    });

    it('still has the behaviours the way out and the signer policy rely on (§1.10, §1.11)', () => {
        const defaults = /const DEFAULT_FEATURES = \[([^\]]*)\];/.exec(WSM_JS)?.[1] ?? '';
        expect(defaults).toContain('SolanaSignAndSendTransaction');
        expect(defaults).toContain('SolanaSignTransaction');
        expect(WSM_JS).toContain('features: account.features ?? DEFAULT_FEATURES');
        expect(WSM_JS).toMatch(/#performReauthorization = async[\s\S]*?\} catch \(e\) \{\s*this\.#disconnect\(\);/);
        expect(WSM_JS).toContain(
            'if (this.#connectionGeneration !== currentConnectionGeneration) await new Promise(() => {});',
        );
        // #disconnect resets the connect lock and never sends `deauthorize` (Disconnect is local).
        expect(WSM_JS).toMatch(
            /#disconnect = async \(\) => \{\s*this\.#authorizationCache\.clear\(\);\s*this\.#connecting = false;\s*this\.#connectionGeneration\+\+;/,
        );
        // No remote (QR) wallet unless the config names a relay or a host authority.
        expect(WSM_JS).toContain('"nostrRelay" in config || config.remoteHostAuthority !== void 0');
    });
});

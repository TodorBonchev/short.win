/** Paths into the repository for contract tests that read the specs and sources. */
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const REPO = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..', '..', '..');

export function readRepo(path: string): string {
    return readFileSync(resolve(REPO, path), 'utf8');
}

export function readRepoBytes(path: string): Uint8Array<ArrayBuffer> {
    return new Uint8Array(readFileSync(resolve(REPO, path)));
}

/**
 * Test helpers (Vitest only): the dumped mainnet fixtures of `tests/fixtures` as `RawAccount`s, by
 * their manifest name, with the decoded values `manifest.json` records for them.
 */
import type { Address } from '@solana/kit';
import { readRepo, readRepoBytes } from '../test/repo.ts';
import type { RawAccount } from './readers.ts';

export interface ManifestAccount {
    readonly name: string;
    readonly address?: string;
    readonly owner?: string;
    readonly file?: string;
    readonly lamports?: number;
    readonly role?: string;
    readonly decoded?: Record<string, unknown> | null;
}

interface Manifest {
    readonly accounts: readonly ManifestAccount[];
    readonly snapshots: readonly { group: string; slot: number; block_time?: number }[];
}

let manifest: Manifest | null = null;

export function fixtureManifest(): Manifest {
    // Integers beyond 2⁵³ (u64 amounts) are kept exact as strings.
    manifest ??= JSON.parse(
        readRepo('tests/fixtures/manifest.json').replace(/([:[,]\s*)(-?\d{16,})(?=\s*[,\]}])/g, '$1"$2"'),
    ) as Manifest;
    return manifest;
}

export function fixture(name: string): ManifestAccount {
    const a = fixtureManifest().accounts.find((x) => x.name === name);
    if (!a) throw new Error(`no fixture ${name}`);
    return a;
}

/** The fixture as an account (`null` for one recorded as absent). */
export function fixtureAccount(name: string): RawAccount | null {
    const a = fixture(name);
    if (!a.file || !a.owner || !a.address) return null;
    return {
        address: a.address as Address,
        owner: a.owner as Address,
        lamports: BigInt(a.lamports ?? 1),
        executable: false,
        data: readRepoBytes(`tests/fixtures/${a.file}`),
    };
}

export function fixtureAddress(name: string): Address {
    const a = fixture(name);
    if (!a.address) throw new Error(`fixture ${name} has no address`);
    return a.address as Address;
}

/** The block time of a fixture group's snapshot (the venues' `now`). */
export function snapshotTime(group: string): bigint {
    const s = fixtureManifest().snapshots.find((x) => x.group === group);
    if (!s?.block_time) throw new Error(`no block time for ${group}`);
    return BigInt(s.block_time);
}

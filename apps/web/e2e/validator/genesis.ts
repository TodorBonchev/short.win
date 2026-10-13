/**
 * Genesis accounts for the E2E validator (FRONTEND.md §10.2), written as `solana account --output
 * json` files into `.e2e/accounts/` and loaded with `--account-dir`:
 *
 * - every generated key funded as a system account (no faucet round trips);
 * - the mock venue's empty `VenueExtras` (`["venue_extras", 6]`), which `initialize_config` creates
 *   only for the four real venues — the same TEST-ONLY state the LiteSVM harness writes
 *   (`Env::install_mock_extras`), so a client that passes the `current` extras set works on the mock;
 * - optionally (`SHORTWIN_E2E_VENUES`, default on) the dumped mainnet venue state of
 *   `tests/fixtures` (Raydium pools and configs, pump / PumpSwap globals, curves and pools, the Pyth
 *   SOL/USD account), for create-market's venue detection on real accounts. The venue programs
 *   themselves are deployed with `--upgradeable-program` (venuePrograms()).
 */
import { mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { VENUE_EXTRAS_DISCRIMINATOR } from '@shortwin/client';
import type { Address } from '@solana/kit';
import { getProgramDerivedAddress } from '@solana/kit';
import { PATHS } from '../env.ts';
import { LAMPORTS_PER_SOL, PROGRAMS } from './chain.ts';
import { KEY_NAMES, loadSecretKey, publicKeyOf } from './keys.ts';

const SYSTEM = '11111111111111111111111111111111';

/** Rent-exempt minimum (3,480 lamports per byte-year, two years, 128 bytes of overhead). */
export function rentExempt(space: number): bigint {
    return BigInt((space + 128) * 3_480 * 2);
}

interface AccountJson {
    readonly pubkey: string;
    readonly account: {
        readonly lamports: number;
        readonly data: readonly [string, 'base64'];
        readonly owner: string;
        readonly executable: boolean;
        readonly rentEpoch: number;
        readonly space: number;
    };
}

function accountJson(pubkey: string, lamports: bigint, owner: string, data: Uint8Array): AccountJson {
    if (lamports > BigInt(Number.MAX_SAFE_INTEGER)) throw new Error(`${pubkey}: lamports beyond 2^53`);
    return {
        pubkey,
        account: {
            lamports: Number(lamports),
            data: [Buffer.from(data).toString('base64'), 'base64'],
            owner,
            executable: false,
            rentEpoch: 0,
            space: data.length,
        },
    };
}

/** SOL each generated key starts with. */
export const FUNDING_SOL: Readonly<Record<string, bigint>> = {
    authority: 100n,
    // lend-claim-pot-chain's 1,000× gap buys ≈ 122,500 SOL into the chain market's pool.
    payer: 400_000n,
    mintAuthority: 10n,
    poolAdmin: 10n,
    seedLender: 100n,
    feeRecipient: 1n,
    governance: 1n,
    pause: 1n,
    coverage: 1n,
};
const WALLET_SOL = 1_000n;

interface ManifestEntry {
    readonly name: string;
    readonly group?: string;
    readonly address: string;
    readonly owner?: string;
    readonly lamports?: number;
    readonly executable?: boolean;
    readonly file?: string;
    readonly files?: Readonly<Record<string, string>>;
    readonly absent?: boolean;
    readonly synthetic?: boolean;
    readonly decoded?: Readonly<Record<string, unknown>>;
    /** The pins of the M10 groups (scripts/dump-fixtures.py). */
    readonly sha256_expected?: string;
    readonly elf_trimmed_sha256_expected?: string;
}

function manifest(): { accounts: ManifestEntry[] } {
    return JSON.parse(readFileSync(join(PATHS.fixtures, 'manifest.json'), 'utf8')) as { accounts: ManifestEntry[] };
}

/** The address of the fixture account `name` (tests/fixtures/manifest.json). */
export function fixtureAddress(name: string): Address {
    const a = manifest().accounts.find((e) => e.name === name);
    if (!a) throw new Error(`no fixture account ${name}`);
    return a.address as Address;
}

/**
 * Groups deployed as programs (or not deployed at all), never loaded as accounts. The M10 groups
 * (scripts/dump-fixtures.py `M10_PROGRAM_GROUPS`: the CPMM redeploy, the mainnet SPL programs,
 * Squads v4, the pump redeploys of 2026-10-02 and 2026-10-08/12) hold program accounts at
 * genesis-program or venue addresses; a validator that runs them deploys them itself (the SPL
 * programs: splPrograms()).
 */
const PROGRAM_GROUPS = new Set([
    'raydium_programs',
    'pump_programs',
    'raydium_cpmm_2026_10',
    'spl_programs',
    'squads_programs',
    'pump_programs_2026_10',
    'pump_programs_2026_10_13',
]);
const SYSVAR_PREFIX = 'Sysvar';

/**
 * The dumped venue programs: `[program id, ELF path, upgrade authority]` for `--upgradeable-program`.
 * CPMM is the 2026-10 redeploy (group `raydium_cpmm_2026_10`, M10 WP-2), the build every CPMM test
 * runs on; its program account and upgrade authority equal `raydium_programs`' 445,763,504 build's.
 * pump, PumpSwap and pump-fees are their 2026-10-02 redeploys (group `pump_programs_2026_10`, the
 * M10 pump revalidation), with the same program accounts and upgrade authority as `pump_programs`'.
 */
export function venuePrograms(): [string, string, string][] {
    const m = manifest();
    const out: [string, string, string][] = [];
    const byName = new Map(m.accounts.map((a) => [a.name, a]));
    for (const [prog, dataName] of [
        ['pump_2026_10_program', 'pump_2026_10_program_data'],
        ['pump_amm_2026_10_program', 'pump_amm_2026_10_program_data'],
        ['pump_fees_2026_10_program', 'pump_fees_2026_10_program_data'],
        ['raydium_amm_v4_program', 'raydium_amm_v4_program_data'],
        ['raydium_cpmm_2026_10_program', 'raydium_cpmm_2026_10_program_data'],
    ] as const) {
        const p = byName.get(prog);
        const d = byName.get(dataName);
        const elf = d?.files?.elf;
        const auth = d?.decoded?.upgrade_authority;
        if (!p || !elf || typeof auth !== 'string') throw new Error(`fixture manifest lacks ${prog}`);
        out.push([p.address, join(PATHS.fixtures, elf), auth]);
    }
    return out;
}

/**
 * The mainnet SPL programs (group `spl_programs`, M10 T-MEMO): Token-2022 for `--upgradeable-program`
 * (`[program id, ELF path, upgrade authority]`) with its pinned trimmed-ELF sha256, and spl-memo v3's
 * pinned account sha256. spl-memo v3 needs no override: solana-test-validator 4.1.2's built-in at
 * `MemoSq4g…` is already mainnet's account (BPFLoader2, the same 74,800 bytes; `checkSplPrograms`
 * holds it to the pin). Its built-in Token-2022 is not mainnet's build.
 */
export function splPrograms(): {
    token2022: [string, string, string];
    token2022TrimmedSha256: string;
    memoSha256: string;
} {
    const byName = new Map(manifest().accounts.map((a) => [a.name, a]));
    const p = byName.get('token_2022_program');
    const d = byName.get('token_2022_program_data');
    const memo = byName.get('spl_memo_v3_program');
    const elf = d?.files?.elf;
    const auth = d?.decoded?.upgrade_authority;
    const trimmed = d?.elf_trimmed_sha256_expected;
    if (!p || !elf || typeof auth !== 'string' || !trimmed || !memo?.sha256_expected) {
        throw new Error(
            'fixture manifest lacks the spl_programs group (scripts/dump-fixtures.py fetch --group spl_programs)',
        );
    }
    return {
        token2022: [p.address, join(PATHS.fixtures, elf), auth],
        token2022TrimmedSha256: trimmed,
        memoSha256: memo.sha256_expected,
    };
}

/** The dumped venue state accounts (deduplicated by address; synthetic, absent and sysvars skipped). */
function fixtureAccounts(): AccountJson[] {
    const m = manifest();
    const byAddress = new Map<string, AccountJson>();
    for (const a of m.accounts) {
        if (a.absent || a.synthetic || (a.group && PROGRAM_GROUPS.has(a.group))) continue;
        if (a.address.startsWith(SYSVAR_PREFIX)) continue;
        // The validator's own native mint stays (WSOL is created at genesis).
        if (a.address === 'So11111111111111111111111111111111111111112') continue;
        let file = a.file;
        let lamports = a.lamports;
        if (!file && a.files) file = Object.values(a.files).find((f) => f.endsWith('.bin'));
        if (lamports === undefined && typeof a.decoded?.lamports === 'number') lamports = a.decoded.lamports;
        if (!file || lamports === undefined || !a.owner) continue;
        const data = new Uint8Array(readFileSync(join(PATHS.fixtures, file)));
        byAddress.set(a.address, accountJson(a.address, BigInt(lamports), a.owner, data));
    }
    return [...byAddress.values()];
}

/** The mock venue's empty `VenueExtras` (`version` 2 @8, `bump` @10, `venue` 6 @11; 6,512 bytes). */
async function mockVenueExtras(): Promise<AccountJson> {
    const [address, bump] = await getProgramDerivedAddress({
        programAddress: PROGRAMS.shortwin,
        seeds: [new TextEncoder().encode('venue_extras'), new Uint8Array([6])],
    });
    const data = new Uint8Array(6_512);
    data.set(VENUE_EXTRAS_DISCRIMINATOR, 0);
    new DataView(data.buffer).setUint16(8, 2, true);
    data[10] = bump;
    data[11] = 6;
    return accountJson(address, rentExempt(data.length), PROGRAMS.shortwin, data);
}

export interface GenesisResult {
    readonly accounts: number;
    readonly keys: Readonly<Record<string, Address>>;
}

/** Generates every key (if missing) and writes the genesis accounts directory. */
export async function writeGenesis(opts: { venues: boolean; mockExtras: boolean }): Promise<GenesisResult> {
    rmSync(PATHS.accounts, { recursive: true, force: true });
    mkdirSync(PATHS.accounts, { recursive: true });
    const accounts: AccountJson[] = [];
    const keys: Record<string, Address> = {};
    for (const name of [...KEY_NAMES.admin, ...KEY_NAMES.wallets]) {
        const pk = publicKeyOf(loadSecretKey(name, true));
        keys[name] = pk;
        const sol = FUNDING_SOL[name] ?? WALLET_SOL;
        accounts.push(accountJson(pk, sol * LAMPORTS_PER_SOL, SYSTEM, new Uint8Array(0)));
    }
    if (opts.mockExtras) accounts.push(await mockVenueExtras());
    if (opts.venues) accounts.push(...fixtureAccounts());
    for (const a of accounts) writeFileSync(join(PATHS.accounts, `${a.pubkey}.json`), JSON.stringify(a));
    return { accounts: accounts.length, keys };
}

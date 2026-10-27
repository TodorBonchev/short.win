/**
 * The E2E `solana-test-validator` (FRONTEND.md §10.2): a fresh ledger in `.e2e/ledger` on the
 * dedicated ports, short.win's test-venue build as an upgradeable program whose upgrade authority
 * is the generated `authority` key (`initialize_config` requires its signature), the mock venue
 * (`complete_curve` is retired with pump's v3 exits, design pump-final §8 / D2: nothing calls it),
 * the dumped venue programs (pump, pump-fees, PumpSwap, Raydium AMM v4 / CPMM)
 * with their mainnet upgrade authorities, and the genesis accounts of `genesis.ts`. Token-2022 is
 * mainnet's build (M10 T-MEMO; `splPrograms()`: the dumped ELF as an upgradeable program with its
 * mainnet upgrade authority), replacing the validator's built-in; `SHORTWIN_BUNDLED_SPL=1` keeps the
 * built-in. spl-memo v3 is the built-in, which in solana-test-validator 4.1.2 is already mainnet's
 * BPFLoader2 account byte for byte; `checkSplPrograms()` holds both to their mainnet pins. SPL Token
 * and the ATA program are the built-ins.
 */
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, rmSync } from 'node:fs';
import { ENV, PATHS, PORTS, programPaths, validatorBinary } from '../env.ts';
import { getAccount, PROGRAMS, programDataOf, rpc } from './chain.ts';
import { splPrograms, venuePrograms, writeGenesis } from './genesis.ts';
import { loadSecretKey, publicKeyOf } from './keys.ts';
import { isAlive, readRuntime, spawnLogged, stopPid, updateRuntime, waitFor } from './process.ts';

const BUILD_HINT =
    'build the programs first: `anchor build --tools-version v1.57 --arch v3 && scripts/build-test-venue.sh` (docs/DEVELOPMENT.md)';

/** Whether a healthy E2E validator answers on the E2E port with short.win deployed. */
export async function validatorHealthy(): Promise<boolean> {
    try {
        const h = await rpc().getHealth().send();
        if (h !== 'ok') return false;
        const p = await getAccount(PROGRAMS.shortwin);
        if (!p?.executable) return false;
        // The first `confirmed` banks can predate the genesis accounts' visibility to preflight.
        const slot = await rpc().getSlot({ commitment: 'confirmed' }).send();
        const firstBank = ENV.bundledSpl ? 0n : splPrograms().minimumBankSlot;
        return slot >= firstBank + 3n;
    } catch {
        return false;
    }
}

export async function startValidator(): Promise<void> {
    const progs = programPaths();
    for (const [what, p] of Object.entries(progs)) {
        if (!existsSync(p)) throw new Error(`missing ${what} ELF ${p}: ${BUILD_HINT}`);
    }
    const prev = readRuntime();
    if (isAlive(prev.validatorPid)) await stopPid(prev.validatorPid);
    rmSync(PATHS.ledger, { recursive: true, force: true });
    mkdirSync(PATHS.ledger, { recursive: true });

    const genesis = await writeGenesis({ venues: ENV.venues, mockExtras: ENV.program === 'test-venue' });
    const authority = publicKeyOf(loadSecretKey('authority'));
    const args = [
        '--reset',
        '--quiet',
        '--ledger',
        PATHS.ledger,
        '--bind-address',
        '127.0.0.1',
        '--rpc-port',
        String(PORTS.rpc),
        '--faucet-port',
        String(PORTS.faucet),
        '--gossip-port',
        String(PORTS.gossip),
        '--dynamic-port-range',
        PORTS.dynamicRange,
        // Keep the whole run's history (the default keeps a few minutes): the `full` profile's
        // indexer backfills it, and specs look landed transactions up.
        '--limit-ledger-size',
        '200000000',
        '--upgradeable-program',
        PROGRAMS.shortwin,
        progs.shortwin,
        authority,
        '--account-dir',
        PATHS.accounts,
    ];
    if (ENV.program === 'test-venue') args.push('--bpf-program', PROGRAMS.mock, progs.mock);
    // A program given on the command line replaces the genesis SPL program at its address.
    if (!ENV.bundledSpl) {
        const spl = splPrograms();
        // A local genesis deployment uses its own header. Warp the execution bank
        // explicitly; this is synthetic qualification, not a live-bank snapshot.
        args.push('--warp-slot', String(spl.minimumBankSlot), '--upgradeable-program', ...spl.token2022);
    }
    if (ENV.venues) {
        for (const [id, elf, auth] of venuePrograms()) args.push('--upgradeable-program', id, elf, auth);
    }
    const child = spawnLogged(validatorBinary(), args, 'validator.log');
    if (!child.pid) throw new Error('solana-test-validator did not start');
    updateRuntime({ validatorPid: child.pid });
    console.log(
        `[e2e] solana-test-validator pid ${child.pid} on :${PORTS.rpc} (${genesis.accounts} genesis accounts, ${
            ENV.program
        } build${ENV.venues ? ', dumped venues' : ''}); log .e2e/logs/validator.log`,
    );
    await waitFor(
        'the E2E validator (see .e2e/logs/validator.log)',
        async () => {
            if (!isAlive(child.pid)) throw new Error('solana-test-validator exited; see .e2e/logs/validator.log');
            return validatorHealthy();
        },
        120_000,
    );
    await checkSplPrograms();
}

/** BPFLoader2: spl-memo v3's loader on mainnet. */
const BPF_LOADER_2 = 'BPFLoader2111111111111111111111111111111111';

function sha256Hex(data: Uint8Array): string {
    return createHash('sha256').update(data).digest('hex');
}

/**
 * Holds the running validator's Token-2022 and spl-memo v3 to the mainnet pins (M10 T-MEMO): the
 * Token-2022 ProgramData after its 45-byte header, trailing zeros trimmed (solana-verify's hash), and
 * the spl-memo v3 account (BPFLoader2, data = ELF; the built-in, a guard against a validator whose
 * built-in differs). A no-op under `SHORTWIN_BUNDLED_SPL=1`.
 */
export async function checkSplPrograms(): Promise<void> {
    if (ENV.bundledSpl) {
        console.log("[e2e] SHORTWIN_BUNDLED_SPL=1: the validator's built-in Token-2022 and spl-memo v3");
        return;
    }
    const spl = splPrograms();
    const slot = await rpc().getSlot({ commitment: 'confirmed' }).send();
    if (slot < spl.minimumBankSlot) {
        throw new Error(
            `Token-2022 qualification bank ${slot} predates ${spl.minimumBankSlot}; restart without SHORTWIN_E2E_REUSE`,
        );
    }
    const pd = await getAccount(await programDataOf(PROGRAMS.token2022));
    const elf = pd?.data.subarray(45) ?? new Uint8Array();
    let end = elf.length;
    while (end > 0 && elf[end - 1] === 0) end--;
    const t22 = sha256Hex(elf.subarray(0, end));
    const memo = await getAccount(PROGRAMS.memo);
    const memoSha = memo ? sha256Hex(memo.data) : 'missing';
    if (t22 !== spl.token2022TrimmedSha256 || memoSha !== spl.memoSha256 || memo?.owner !== BPF_LOADER_2) {
        throw new Error(
            `the E2E validator does not run the mainnet SPL programs: Token-2022 ProgramData trimmed sha256 ${t22} (want ${spl.token2022TrimmedSha256}), spl-memo v3 ${memoSha} owner ${memo?.owner} (want ${spl.memoSha256} under ${BPF_LOADER_2}); restart it without SHORTWIN_E2E_REUSE`,
        );
    }
    console.log(
        `[e2e] mainnet SPL programs: Token-2022 ProgramData trimmed sha256 ${t22}, spl-memo v3 ${memoSha} (${BPF_LOADER_2})`,
    );
}

export async function stopValidator(): Promise<void> {
    const r = readRuntime();
    await stopPid(r.validatorPid);
    updateRuntime({ validatorPid: undefined });
}

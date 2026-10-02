/**
 * The E2E `solana-test-validator` (FRONTEND.md §10.2): a fresh ledger in `.e2e/ledger` on the
 * dedicated ports, short.win's test-venue build as an upgradeable program whose upgrade authority
 * is the generated `authority` key (`initialize_config` requires its signature), the mock venue and
 * `complete_curve`, the dumped venue programs (pump, pump-fees, PumpSwap, Raydium AMM v4 / CPMM)
 * with their mainnet upgrade authorities, and the genesis accounts of `genesis.ts`. Token,
 * Token-2022, the ATA program and spl-memo are the validator's built-ins.
 */
import { existsSync, mkdirSync, rmSync } from 'node:fs';
import { ENV, PATHS, PORTS, programPaths, validatorBinary } from '../env.ts';
import { getAccount, PROGRAMS, rpc } from './chain.ts';
import { venuePrograms, writeGenesis } from './genesis.ts';
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
        return slot >= 3n;
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
        '--bpf-program',
        PROGRAMS.completeCurve,
        progs.completeCurve,
        '--account-dir',
        PATHS.accounts,
    ];
    if (ENV.program === 'test-venue') args.push('--bpf-program', PROGRAMS.mock, progs.mock);
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
}

export async function stopValidator(): Promise<void> {
    const r = readRuntime();
    await stopPid(r.validatorPid);
    updateRuntime({ validatorPid: undefined });
}

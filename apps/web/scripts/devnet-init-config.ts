/**
 * One-shot, devnet-only ShortWin 1.0.3 configuration initialization.
 *
 * Planning is unsigned and never opens a keypair file. --send requires both the
 * private authority path and the hash printed by a preceding successful plan.
 * This is deliberately separate from shortwin-gov: public devnet Squads is
 * mutable, while this tool installs explicitly labelled direct devnet keys.
 */
import { createHash } from 'node:crypto';
import { lstatSync, readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';
import {
    findConfigPda,
    findOracleCachePda,
    findVenueExtrasPumpCurvePda,
    findVenueExtrasPumpSwapPda,
    findVenueExtrasRaydiumCpmmPda,
    findVenueExtrasRaydiumV4Pda,
    findVenueGuardPda,
    getInitializeConfigInstructionAsync,
    SHORTWIN_PROGRAM_ADDRESS,
} from '@shortwin/client';
import {
    type Address,
    address,
    appendTransactionMessageInstructions,
    compileTransaction,
    createKeyPairSignerFromBytes,
    createNoopSigner,
    createSolanaRpc,
    createTransactionMessage,
    getAddressDecoder,
    getAddressEncoder,
    getBase64EncodedWireTransaction,
    getProgramDerivedAddress,
    getSignatureFromTransaction,
    getTransactionSize,
    setTransactionMessageFeePayerSigner,
    setTransactionMessageLifetimeUsingBlockhash,
    signTransactionMessageWithSigners,
    type TransactionSigner,
} from '@solana/kit';
import { defaultParams } from '../e2e/validator/params.ts';

const DEVNET_GENESIS = 'EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG';
const RPC_DEFAULT = 'https://api.devnet.solana.com';
const LOADER = address('BPFLoaderUpgradeab1e11111111111111111111111');
const PYTH = address('7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE');
const PYTH_RECEIVER = address('rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ');
const CLOCK = address('SysvarC1ock11111111111111111111111111111111');
const FEED_ID = Buffer.from('ef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d', 'hex');
const VENUES = [
    address('6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P'),
    address('pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA'),
    address('pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ'),
    address('675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8'),
    address('CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C'),
    address('JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4'),
] as const;
const TX_MAX_BYTES = 1232;
const DEFAULT_MAX_DEBIT = 200_000_000n;
const DEFAULT_TIMELOCK = 172_800;

type RpcAccount = {
    owner: Address;
    executable: boolean;
    lamports: bigint | number;
    data: readonly [string, string];
};

export interface Args {
    readonly rpcUrl: string;
    readonly upgradeAuthority: Address;
    readonly governance: Address;
    readonly pauseAuthority: Address;
    readonly coverageAuthority: Address;
    readonly feeRecipient: Address;
    readonly timelockS: number;
    readonly venueBits: number;
    readonly maxDebitLamports: bigint;
    readonly send: boolean;
    readonly expectedPlanSha256?: string;
    readonly authorityKeypair?: string;
}

const USAGE = `node apps/web/scripts/devnet-init-config.ts
  --upgrade-authority <public-key>
  --governance <public-key> --pause-authority <public-key>
  --coverage-authority <public-key> --fee-recipient <public-key>
  [--rpc-url https://api.devnet.solana.com] [--venue-bits 0|1|2|3]
  [--timelock-s 172800] [--max-debit-lamports 200000000]
  [--send --expected-plan-sha256 <sha256> --authority-keypair <outside-repo-json>]

The default plan uses zero enabled venues and performs unsigned simulation.
--send requires a reviewed plan hash and a successful signed simulation.`;

function decimal(s: string, label: string): bigint {
    if (!/^(0|[1-9][0-9]*)$/.test(s)) throw new Error(`${label} must be a decimal integer`);
    return BigInt(s);
}

export function parseArgs(argv: readonly string[], env: NodeJS.ProcessEnv = process.env): Args {
    if (argv.includes('--help') || argv.includes('-h')) throw new Error(USAGE);
    const options = new Map<string, string>();
    let send = false;
    const values = new Set([
        '--upgrade-authority',
        '--governance',
        '--pause-authority',
        '--coverage-authority',
        '--fee-recipient',
        '--rpc-url',
        '--venue-bits',
        '--timelock-s',
        '--max-debit-lamports',
        '--expected-plan-sha256',
        '--authority-keypair',
    ]);
    for (let i = 0; i < argv.length; i++) {
        const flag = argv[i];
        if (flag === '--send') {
            if (send) throw new Error('duplicate --send');
            send = true;
        } else if (values.has(flag)) {
            const value = argv[++i];
            if (!value || value.startsWith('--') || options.has(flag)) throw new Error(`missing or duplicate ${flag}`);
            options.set(flag, value);
        } else {
            throw new Error(`unknown option ${flag}\n${USAGE}`);
        }
    }
    const required = (k: string): string => {
        const v = options.get(k);
        if (!v) throw new Error(`${k} is required\n${USAGE}`);
        return v;
    };
    const timelock = Number(decimal(options.get('--timelock-s') ?? String(DEFAULT_TIMELOCK), '--timelock-s'));
    if (!Number.isSafeInteger(timelock) || timelock < 86_400 || timelock > 2_592_000) {
        throw new Error('--timelock-s is outside the compiled 86400..2592000 bound');
    }
    const venueBits = Number(decimal(options.get('--venue-bits') ?? '0', '--venue-bits'));
    if (![0, 1, 2, 3].includes(venueBits)) throw new Error('only Pump/PumpSwap venue bits 0..3 are allowed');
    const maxDebitLamports = decimal(
        options.get('--max-debit-lamports') ?? String(DEFAULT_MAX_DEBIT),
        '--max-debit-lamports',
    );
    if (maxDebitLamports < 150_000_000n || maxDebitLamports > 1_000_000_000n) {
        throw new Error('--max-debit-lamports must be within 150000000..1000000000');
    }
    const rpcUrl = options.get('--rpc-url') ?? RPC_DEFAULT;
    const u = new URL(rpcUrl);
    if (!['https:', 'http:'].includes(u.protocol) || u.username || u.password) throw new Error('invalid --rpc-url');
    if (u.protocol === 'http:' && !['localhost', '127.0.0.1'].includes(u.hostname)) {
        throw new Error('non-local --rpc-url must use HTTPS');
    }
    const roles = {
        upgradeAuthority: address(required('--upgrade-authority')),
        governance: address(required('--governance')),
        pauseAuthority: address(required('--pause-authority')),
        coverageAuthority: address(required('--coverage-authority')),
        feeRecipient: address(required('--fee-recipient')),
    };
    const roleKeys = [roles.governance, roles.pauseAuthority, roles.coverageAuthority, roles.feeRecipient];
    if (new Set(roleKeys).size !== roleKeys.length)
        throw new Error('the four on-chain authority roles must be distinct');
    const expectedPlanSha256 = options.get('--expected-plan-sha256');
    if (expectedPlanSha256 && !/^[0-9a-f]{64}$/.test(expectedPlanSha256))
        throw new Error('invalid --expected-plan-sha256');
    const authorityKeypair = options.get('--authority-keypair') ?? env.SHORTWIN_DEVNET_AUTHORITY_KEYPAIR;
    if (send && (!expectedPlanSha256 || !authorityKeypair)) {
        throw new Error(
            '--send requires --expected-plan-sha256 and --authority-keypair (or SHORTWIN_DEVNET_AUTHORITY_KEYPAIR)',
        );
    }
    if (!send && (expectedPlanSha256 || options.has('--authority-keypair'))) {
        throw new Error('plan mode does not accept a keypair path or expected plan hash');
    }
    return {
        rpcUrl,
        ...roles,
        timelockS: timelock,
        venueBits,
        maxDebitLamports,
        send,
        expectedPlanSha256,
        authorityKeypair,
    };
}

function account(v: unknown, label: string): RpcAccount | null {
    if (v === null) return null;
    if (typeof v !== 'object' || !v || !('owner' in v) || !('data' in v))
        throw new Error(`${label}: malformed RPC account`);
    const a = v as RpcAccount;
    if (!Array.isArray(a.data) || a.data[1] !== 'base64' || typeof a.data[0] !== 'string') {
        throw new Error(`${label}: unexpected RPC account encoding`);
    }
    return a;
}

function data(a: RpcAccount): Buffer {
    return Buffer.from(a.data[0], 'base64');
}

function readProgram(
    program: RpcAccount | null,
    programData: RpcAccount | null,
    pdAddress: Address,
    label: string,
): bigint | null {
    if (!program || !programData) return null;
    // Disabled public-devnet venues may hold non-executable placeholder accounts.
    // Callers require nonzero slots for every venue they intend to enable.
    if (!program.executable || program.owner !== LOADER || programData.owner !== LOADER) return null;
    const p = data(program);
    const pd = data(programData);
    if (p.length < 36 || p.readUInt32LE(0) !== 2 || getAddressDecoder().decode(p.subarray(4, 36)) !== pdAddress) {
        throw new Error(`${label}: Program account does not point to expected ProgramData`);
    }
    if (pd.length < 45 || pd.readUInt32LE(0) !== 3 || ![0, 1].includes(pd[12])) {
        throw new Error(`${label}: malformed ProgramData`);
    }
    return pd.readBigUInt64LE(4);
}

export function readPyth(a: RpcAccount, clockUnix: bigint, maxAgeS: number, maxConfBps: number) {
    if (a.owner !== PYTH_RECEIVER) throw new Error('Pyth account has wrong owner');
    const b = data(a);
    if (b.length < 133 || b.subarray(0, 8).toString('hex') !== '22f123639d7ef4cd' || b[40] !== 1) {
        throw new Error('Pyth PriceUpdateV2 is not a full verified update');
    }
    if (!b.subarray(41, 73).equals(FEED_ID)) throw new Error('Pyth account has wrong SOL/USD feed');
    const price = b.readBigInt64LE(73);
    const conf = b.readBigUInt64LE(81);
    const exponent = b.readInt32LE(89);
    const publishTime = b.readBigInt64LE(93);
    if (exponent < -18 || exponent > 0 || price <= 0n) throw new Error('Pyth update has invalid exponent or price');
    if (conf * 10_000n > BigInt(maxConfBps) * price) throw new Error('Pyth confidence exceeds protocol limit');
    const ageS = clockUnix - publishTime;
    if (ageS > BigInt(maxAgeS) || ageS < -BigInt(maxAgeS))
        throw new Error(`Pyth SOL/USD update outside ±${maxAgeS}s: age ${ageS}s`);
    return {
        price: price.toString(),
        conf: conf.toString(),
        exponent,
        publishTime: publishTime.toString(),
        ageS: ageS.toString(),
    };
}

async function loadSigner(path: string, expected: Address) {
    const st = lstatSync(path);
    if (!st.isFile() || (st.mode & 0o077) !== 0) throw new Error('authority keypair must be a regular mode-0600 file');
    const raw: unknown = JSON.parse(readFileSync(path, 'utf8'));
    if (!Array.isArray(raw) || raw.length !== 64 || raw.some((n) => !Number.isInteger(n) || n < 0 || n > 255)) {
        throw new Error('authority keypair file is not a Solana 64-byte JSON keypair');
    }
    const signer = await createKeyPairSignerFromBytes(new Uint8Array(raw as number[]));
    if (signer.address !== expected) throw new Error('authority keypair public key does not match --upgrade-authority');
    return signer;
}

function messageFor(
    signer: TransactionSigner,
    ix: Awaited<ReturnType<typeof getInitializeConfigInstructionAsync>>,
    lifetime: {
        blockhash: string;
        lastValidBlockHeight: bigint;
    },
) {
    // initialize_config is 1,198 B in legacy form. An outer compute-budget
    // instruction or v0 overhead can exceed Solana's 1,232-byte packet limit.
    const m0 = createTransactionMessage({ version: 'legacy' });
    const m1 = setTransactionMessageFeePayerSigner(signer, m0);
    const m2 = setTransactionMessageLifetimeUsingBlockhash(
        lifetime as Parameters<typeof setTransactionMessageLifetimeUsingBlockhash>[0],
        m1,
    );
    return appendTransactionMessageInstructions([ix], m2);
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

async function main() {
    const a = parseArgs(process.argv.slice(2));
    const rpc = createSolanaRpc(a.rpcUrl);
    const genesis = await rpc.getGenesisHash().send();
    if (genesis !== DEVNET_GENESIS) throw new Error(`RPC genesis ${genesis} is not Solana devnet`);
    const [programData] = await getProgramDerivedAddress({
        programAddress: LOADER,
        seeds: [getAddressEncoder().encode(SHORTWIN_PROGRAM_ADDRESS)],
    });
    const venueProgramData = await Promise.all(
        VENUES.map(
            async (v) =>
                (
                    await getProgramDerivedAddress({
                        programAddress: LOADER,
                        seeds: [getAddressEncoder().encode(v)],
                    })
                )[0],
        ),
    );
    const created = await Promise.all([
        findConfigPda(),
        findVenueGuardPda(),
        findVenueExtrasPumpCurvePda(),
        findVenueExtrasPumpSwapPda(),
        findVenueExtrasRaydiumV4Pda(),
        findVenueExtrasRaydiumCpmmPda(),
        findOracleCachePda(),
    ]).then((all) => all.map(([key]) => key));
    const addresses = [SHORTWIN_PROGRAM_ADDRESS, programData, ...VENUES, ...venueProgramData, PYTH, CLOCK, ...created];
    const { context, value } = await rpc
        .getMultipleAccounts(addresses, { encoding: 'base64', commitment: 'confirmed' })
        .send();
    const byAddress = new Map(addresses.map((key, i) => [key, account(value[i], key)]));
    const program = byAddress.get(SHORTWIN_PROGRAM_ADDRESS) ?? null;
    const pd = byAddress.get(programData) ?? null;
    const deploySlot = readProgram(program, pd, programData, 'ShortWin');
    if (deploySlot === null || !pd) throw new Error('ShortWin program/ProgramData absent');
    const pdBytes = data(pd);
    if (pdBytes[12] !== 1 || getAddressDecoder().decode(pdBytes.subarray(13, 45)) !== a.upgradeAuthority) {
        throw new Error('ShortWin ProgramData upgrade authority differs from --upgrade-authority');
    }
    for (const key of created)
        if (byAddress.get(key))
            throw new Error(`configuration PDA ${key} already exists; refusing one-shot initialization`);
    const guardSlots = VENUES.map(
        (key, i) =>
            readProgram(
                byAddress.get(key) ?? null,
                byAddress.get(venueProgramData[i]) ?? null,
                venueProgramData[i],
                `venue ${i}`,
            ) ?? 0n,
    );
    if (a.venueBits !== 0 && guardSlots.slice(0, 3).some((s) => s === 0n)) {
        throw new Error('Pump, PumpSwap and pump-fees executables must all be present before enabling a Pump venue');
    }
    const p = defaultParams();
    const clockAccount = byAddress.get(CLOCK);
    const pythAccount = byAddress.get(PYTH);
    if (!clockAccount || data(clockAccount).length < 40 || !pythAccount)
        throw new Error('Clock or Pyth SOL/USD missing');
    const clockUnix = data(clockAccount).readBigInt64LE(32);
    const oracle = readPyth(pythAccount, clockUnix, p.params.oracleMaxAgeS, p.params.oracleMaxConfBps);
    const ixArgs = {
        programData,
        governance: a.governance,
        pauseAuthority: a.pauseAuthority,
        coverageAuthority: a.coverageAuthority,
        feeRecipient: a.feeRecipient,
        pythSolUsd: PYTH,
        pythFeedId: FEED_ID,
        timelockS: a.timelockS,
        venueEnabled: a.venueBits,
        params: p.params,
        tiers: p.tiers,
        ema: p.ema,
        utilKinkBps: p.utilKinkBps,
        guardSlot: guardSlots,
    };
    const noop = createNoopSigner(a.upgradeAuthority);
    const ix = await getInitializeConfigInstructionAsync({ authority: noop, ...ixArgs });
    const planSha256 = createHash('sha256')
        .update(
            JSON.stringify({
                genesis,
                program: SHORTWIN_PROGRAM_ADDRESS,
                payer: a.upgradeAuthority,
                instructionData: Buffer.from(ix.data).toString('base64'),
                accounts: ix.accounts?.map((meta) => ({ address: meta.address, role: meta.role })),
            }),
        )
        .digest('hex');
    const { value: lifetime } = await rpc.getLatestBlockhash({ commitment: 'confirmed' }).send();
    const unsigned = compileTransaction(messageFor(noop, ix, lifetime));
    const unsignedWire = getBase64EncodedWireTransaction(unsigned);
    const wireBytes = getTransactionSize(unsigned);
    if (wireBytes > TX_MAX_BYTES) throw new Error(`transaction is ${wireBytes} bytes, over ${TX_MAX_BYTES}`);
    const { value: sim } = await rpc
        .simulateTransaction(unsignedWire, {
            encoding: 'base64',
            sigVerify: false,
            replaceRecentBlockhash: true,
            commitment: 'confirmed',
            accounts: { addresses: [a.upgradeAuthority, ...created], encoding: 'base64' },
        })
        .send();
    if (sim.err)
        throw new Error(
            `unsigned simulation failed: ${JSON.stringify(sim.err, (_, v: unknown) => (typeof v === 'bigint' ? v.toString() : v))}\n${(sim.logs ?? []).slice(-10).join('\n')}`,
        );
    const payer =
        byAddress.get(a.upgradeAuthority) ??
        account(
            (await rpc.getAccountInfo(a.upgradeAuthority, { encoding: 'base64', commitment: 'confirmed' }).send())
                .value,
            'payer',
        );
    if (!payer || !sim.accounts?.[0]) throw new Error('payer account or simulated post-balance missing');
    const simulatedDebit = BigInt(payer.lamports) - BigInt(sim.accounts[0].lamports);
    if (simulatedDebit < 0n || simulatedDebit + 5_000n > a.maxDebitLamports) {
        throw new Error(
            `simulated debit ${simulatedDebit} plus base fee exceeds --max-debit-lamports ${a.maxDebitLamports}`,
        );
    }
    if (sim.accounts.slice(1).some((x) => !x)) throw new Error('simulation did not create all seven PDAs');
    const summary = {
        cluster: 'devnet',
        genesis,
        rpcContextSlot: String(context.slot),
        program: SHORTWIN_PROGRAM_ADDRESS,
        programDeploySlot: String(deploySlot),
        feePayerAndSigner: a.upgradeAuthority,
        recipientAccounts: created,
        governance: a.governance,
        pauseAuthority: a.pauseAuthority,
        coverageAuthority: a.coverageAuthority,
        feeRecipientStoredOnly: a.feeRecipient,
        venueBits: a.venueBits,
        timelockS: a.timelockS,
        venueGuardSlots: guardSlots.map(String),
        oracle,
        transactionVersion: 'legacy',
        wireBytes,
        computeUnitLimit: 'runtime default',
        simulatedUnits: String(sim.unitsConsumed ?? 'unknown'),
        simulatedPayerDebitLamports: String(simulatedDebit),
        maxDebitLamports: String(a.maxDebitLamports),
        token: 'devnet SOL (rent and fees only)',
        planSha256,
    };
    console.log(JSON.stringify(summary, null, 2));
    if (!a.send) {
        console.log('UNSIGNED SIMULATION PASSED; no keypair opened, no transaction signed or sent.');
        return;
    }
    if (a.expectedPlanSha256 !== planSha256)
        throw new Error('plan hash changed; rerun unsigned plan and review before sending');
    if (!a.authorityKeypair) throw new Error('missing authority keypair path');
    const signer = await loadSigner(a.authorityKeypair, a.upgradeAuthority);
    const signedIx = await getInitializeConfigInstructionAsync({ authority: signer, ...ixArgs });
    const sameAccounts =
        JSON.stringify(signedIx.accounts?.map((m) => [m.address, m.role])) ===
        JSON.stringify(ix.accounts?.map((m) => [m.address, m.role]));
    if (!Buffer.from(signedIx.data).equals(Buffer.from(ix.data)) || !sameAccounts) {
        throw new Error('signed instruction differs from the simulated plan');
    }
    const signedMessage = messageFor(signer, signedIx, lifetime);
    const signed = await signTransactionMessageWithSigners(signedMessage);
    if (getTransactionSize(signed) !== wireBytes) throw new Error('signed wire size differs from the simulated plan');
    const signedWire = getBase64EncodedWireTransaction(signed);
    const { value: signedSim } = await rpc
        .simulateTransaction(signedWire, {
            encoding: 'base64',
            sigVerify: true,
            commitment: 'confirmed',
            accounts: { addresses: [a.upgradeAuthority, ...created], encoding: 'base64' },
        })
        .send();
    if (signedSim.err)
        throw new Error(
            `signed simulation failed: ${JSON.stringify(signedSim.err, (_, v: unknown) => (typeof v === 'bigint' ? v.toString() : v))}\n${(signedSim.logs ?? []).slice(-10).join('\n')}`,
        );
    if (!signedSim.accounts?.[0]) throw new Error('signed simulation did not return the payer balance');
    const signedDebit = BigInt(payer.lamports) - BigInt(signedSim.accounts[0].lamports);
    if (signedDebit < 0n || signedDebit + 5_000n > a.maxDebitLamports) {
        throw new Error(`signed simulation debit ${signedDebit} exceeds --max-debit-lamports`);
    }
    const expectedSignature = getSignatureFromTransaction(signed);
    const signature = await rpc
        .sendTransaction(signedWire, {
            encoding: 'base64',
            preflightCommitment: 'confirmed',
            skipPreflight: false,
            maxRetries: 3n,
        })
        .send();
    if (signature !== expectedSignature)
        throw new Error(`RPC returned unexpected signature; expected ${expectedSignature}, got ${signature}`);
    console.log(`sent devnet initialize_config: ${signature}`);
    const deadline = Date.now() + 90_000;
    for (;;) {
        const { value: statuses } = await rpc
            .getSignatureStatuses([signature], { searchTransactionHistory: true })
            .send();
        const status = statuses[0];
        if (status?.err)
            throw new Error(`on-chain initialization failed: ${JSON.stringify(status.err)}; signature ${signature}`);
        if (status?.confirmationStatus === 'finalized') break;
        if (Date.now() >= deadline)
            throw new Error(`not finalized within 90s; inspect signature ${signature} before any retry`);
        await sleep(1_500);
    }
    const { value: config } = await rpc
        .getAccountInfo(created[0], { encoding: 'base64', commitment: 'finalized' })
        .send();
    if (!config || config.owner !== SHORTWIN_PROGRAM_ADDRESS)
        throw new Error(`finalized signature ${signature} but GlobalConfig is missing or wrong owner`);
    console.log(`finalized devnet initialize_config: ${signature}; GlobalConfig ${created[0]} verified`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
    main().catch((e: unknown) => {
        console.error(e instanceof Error ? e.message : String(e));
        process.exitCode = 1;
    });
}

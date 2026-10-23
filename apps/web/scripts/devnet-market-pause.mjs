/** Emergency market-scoped open gate for the bounded devnet trading smoke. */
import { readFileSync, statSync } from 'node:fs';
import { pathToFileURL } from 'node:url';
import {
    findConfigPda,
    findMarketPda,
    GLOBAL_CONFIG_DISCRIMINATOR,
    getGlobalConfigDecoder,
    getMarketDecoder,
    getSetPauseInstructionAsync,
    MARKET_DISCRIMINATOR,
    PauseScope,
    SHORTWIN_PROGRAM_ADDRESS,
} from '@shortwin/client';
import {
    address,
    appendTransactionMessageInstructions,
    compileTransaction,
    createKeyPairSignerFromBytes,
    createSolanaRpc,
    createTransactionMessage,
    getBase64EncodedWireTransaction,
    getProgramDerivedAddress,
    getSignatureFromTransaction,
    getTransactionSize,
    setTransactionMessageFeePayerSigner,
    setTransactionMessageLifetimeUsingBlockhash,
    signTransactionMessageWithSigners,
} from '@solana/kit';

const DEVNET_GENESIS = 'EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG';
const MINT = address('2isJ4phZS9Nv1fVHRLnjGkgvrFW4cb13qa8Y6kknpump');
const MARKET = address('tmRs5cmSphuftivEwfKr3QMRVdGynZMHqWwki2qnD3H');
const PAUSE_AUTHORITY = address('GHcdeRUzPMLtCPdCoZ3NuGsJ422NmKK1WjRUpXe3a6mu');
const FEE_PAYER = address('35DeTJZZKgHzbLfWe9yiM1EwFfVUTcPCFSizLd8crDbc');
const LOADER = address('BPFLoaderUpgradeab1e11111111111111111111111');
const PAUSED_OPENS = 1;
const DEFAULT_RPC = 'https://api.devnet.solana.com';

function options(argv) {
    const out = { send: false };
    const allowed = new Set(['mode', 'authority-keypair', 'payer-keypair', 'max-spend-lamports', 'rpc']);
    for (let i = 0; i < argv.length; i += 1) {
        const flag = argv[i];
        if (flag === '--send') {
            if (out.send) throw new Error('duplicate --send');
            out.send = true;
            continue;
        }
        const key = flag?.slice(2);
        if (!flag?.startsWith('--') || !allowed.has(key) || out[key] !== undefined) {
            throw new Error(`unknown or duplicate option ${flag}`);
        }
        const value = argv[++i];
        if (!value || value.startsWith('--')) throw new Error(`${flag} needs a value`);
        out[key] = value;
    }
    if (!['on', 'off'].includes(out.mode)) throw new Error('--mode on|off required');
    if (!out['authority-keypair'] || !out['payer-keypair']) throw new Error('both keypair paths are required');
    if (!/^[1-9][0-9]*$/.test(out['max-spend-lamports'] ?? '')) {
        throw new Error('--max-spend-lamports must be positive');
    }
    const cap = BigInt(out['max-spend-lamports']);
    if (cap > 1_000_000n) throw new Error('fee debit cap exceeds 0.001 devnet SOL');
    out.rpc ??= DEFAULT_RPC;
    if (!/^https:\/\/|^http:\/\/127\.0\.0\.1(?::\d+)?\/?$/.test(out.rpc) || out.rpc.includes('mainnet')) {
        throw new Error('only HTTPS or loopback devnet RPC is allowed');
    }
    return { ...out, cap, paused: out.mode === 'on' };
}

async function signerFromFile(file) {
    if ((statSync(file).mode & 0o077) !== 0) throw new Error('keypair file must be mode 0600');
    const bytes = JSON.parse(readFileSync(file, 'utf8'));
    if (!Array.isArray(bytes) || bytes.length !== 64 || bytes.some((n) => !Number.isInteger(n) || n < 0 || n > 255)) {
        throw new Error('expected a Solana CLI 64-byte JSON keypair');
    }
    return createKeyPairSignerFromBytes(new Uint8Array(bytes));
}

function owned(raw, owner, discriminator, decoder, label) {
    if (!raw || raw.owner !== owner) throw new Error(`${label} is absent or owned by another program`);
    const data = Buffer.from(raw.data[0], 'base64');
    if (!data.subarray(0, 8).equals(Buffer.from(discriminator))) throw new Error(`${label} discriminator mismatch`);
    return decoder.decode(data);
}

async function main(argv = process.argv.slice(2)) {
    const opts = options(argv);
    const [authority, payer] = await Promise.all([
        signerFromFile(opts['authority-keypair']),
        signerFromFile(opts['payer-keypair']),
    ]);
    if (authority.address !== PAUSE_AUTHORITY || payer.address !== FEE_PAYER || authority.address === payer.address) {
        throw new Error('authority or fee payer does not match the staged devnet role');
    }
    const rpc = createSolanaRpc(opts.rpc);
    if ((await rpc.getGenesisHash().send()) !== DEVNET_GENESIS) throw new Error('non-devnet RPC refused');
    const [configKey] = await findConfigPda();
    const [marketKey] = await findMarketPda({ mint: MINT });
    if (marketKey !== MARKET) throw new Error('expected market PDA mismatch');
    const {
        value: [programRaw, configRaw, marketRaw],
    } = await rpc
        .getMultipleAccounts([SHORTWIN_PROGRAM_ADDRESS, configKey, marketKey], {
            encoding: 'base64',
            commitment: 'confirmed',
        })
        .send();
    if (!programRaw?.executable || programRaw.owner !== LOADER) throw new Error('program executable missing');
    const config = owned(
        configRaw,
        SHORTWIN_PROGRAM_ADDRESS,
        GLOBAL_CONFIG_DISCRIMINATOR,
        getGlobalConfigDecoder(),
        'config',
    );
    const market = owned(marketRaw, SHORTWIN_PROGRAM_ADDRESS, MARKET_DISCRIMINATOR, getMarketDecoder(), 'market');
    if (
        config.pauseAuthority !== authority.address ||
        market.mint !== MINT ||
        market.venue !== 1 ||
        market.tier !== 3
    ) {
        throw new Error('config authority or market identity mismatch');
    }
    const beforePaused = (market.flags & PAUSED_OPENS) !== 0;
    if (beforePaused === opts.paused) throw new Error(`market already has paused_opens=${opts.paused}`);
    if (!opts.paused && market.openPositions !== 0) throw new Error('refusing to clear while a position is open');
    const [eventAuthority] = await getProgramDerivedAddress({
        programAddress: SHORTWIN_PROGRAM_ADDRESS,
        seeds: ['__event_authority'],
    });
    const ix = await getSetPauseInstructionAsync({
        authority,
        market: marketKey,
        eventAuthority,
        program: SHORTWIN_PROGRAM_ADDRESS,
        scope: PauseScope.Market,
        paused: opts.paused,
    });
    const { value: lifetime } = await rpc.getLatestBlockhash({ commitment: 'confirmed' }).send();
    const message = appendTransactionMessageInstructions(
        [ix],
        setTransactionMessageLifetimeUsingBlockhash(
            lifetime,
            setTransactionMessageFeePayerSigner(payer, createTransactionMessage({ version: 0 })),
        ),
    );
    const unsigned = compileTransaction(message);
    const bytes = getTransactionSize(unsigned);
    if (bytes > 1232) throw new Error('transaction exceeds packet limit');
    const payerBefore = (await rpc.getBalance(payer.address, { commitment: 'confirmed' }).send()).value;
    const sim = (
        await rpc
            .simulateTransaction(getBase64EncodedWireTransaction(unsigned), {
                encoding: 'base64',
                sigVerify: false,
                replaceRecentBlockhash: true,
                commitment: 'confirmed',
                accounts: { addresses: [payer.address, marketKey], encoding: 'base64' },
            })
            .send()
    ).value;
    if (sim.err !== null)
        throw new Error(`simulation failed: ${JSON.stringify(sim.err)}\n${(sim.logs ?? []).slice(-15).join('\n')}`);
    const simulatedPayer = sim.accounts?.[0];
    const simulatedMarket = sim.accounts?.[1];
    if (!simulatedPayer || !simulatedMarket) throw new Error('simulation omitted watched accounts');
    const marketAfter = owned(
        simulatedMarket,
        SHORTWIN_PROGRAM_ADDRESS,
        MARKET_DISCRIMINATOR,
        getMarketDecoder(),
        'simulated market',
    );
    if (((marketAfter.flags & PAUSED_OPENS) !== 0) !== opts.paused || marketAfter.mint !== MINT) {
        throw new Error('simulation did not produce the expected market pause flag');
    }
    const debit = payerBefore > simulatedPayer.lamports ? payerBefore - simulatedPayer.lamports : 0n;
    if (debit + 50_000n > opts.cap || simulatedPayer.lamports < 10_000_000n) {
        throw new Error('simulated payer debit exceeds cap or reserve');
    }
    console.log(
        JSON.stringify(
            {
                cluster: 'devnet',
                market: marketKey,
                mint: MINT,
                instruction: 'set_pause',
                scope: 'market',
                pausedBefore: beforePaused,
                pausedAfter: opts.paused,
                authority: authority.address,
                feePayer: payer.address,
                simulatedDebitLamports: debit.toString(),
                maxSpendLamports: opts.cap.toString(),
                packetBytes: bytes,
                computeUnits: sim.unitsConsumed?.toString() ?? null,
                mode: opts.send ? 'send after simulation' : 'simulation only',
            },
            null,
            2,
        ),
    );
    if (!opts.send) return;
    const signed = await signTransactionMessageWithSigners(message);
    const signature = getSignatureFromTransaction(signed);
    console.log(`signed transaction ${signature}; submitting to devnet`);
    const sent = await rpc
        .sendTransaction(getBase64EncodedWireTransaction(signed), {
            encoding: 'base64',
            skipPreflight: false,
            preflightCommitment: 'confirmed',
            maxRetries: 2n,
        })
        .send();
    if (sent !== signature) throw new Error(`RPC returned a different signature; check ${signature}`);
    for (let i = 0; i < 30; i += 1) {
        const status = (await rpc.getSignatureStatuses([signature], { searchTransactionHistory: true }).send())
            .value[0];
        if (status?.err) throw new Error(`transaction failed ${signature}: ${JSON.stringify(status.err)}`);
        if (status?.confirmationStatus === 'confirmed' || status?.confirmationStatus === 'finalized') {
            console.log(`confirmed ${signature} at slot ${status.slot}`);
            return;
        }
        await new Promise((resolve) => setTimeout(resolve, 1_000));
    }
    throw new Error(`confirmation unknown for ${signature}; check chain state before retrying`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
    main().catch((error) => {
        console.error(`devnet market pause: ${String(error?.message ?? error)}`);
        process.exitCode = 1;
    });
}

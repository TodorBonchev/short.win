/**
 * Bounded, devnet-only operator smoke for one controlled Pump curve market.
 *
 * Simulates by default. --send is required for each individual transaction and still
 * simulates first. The signer is read from an existing 0600 Solana CLI keypair file;
 * its bytes are never printed or written here. This is not an unattended trader.
 *
 * Run from apps/web with Node 24 after pnpm install:
 *   node scripts/devnet-trade-smoke.mjs --help
 */
import { randomBytes } from 'node:crypto';
import { readFileSync, statSync } from 'node:fs';
import { pathToFileURL } from 'node:url';
import {
    findConfigPda,
    findLenderPda,
    findMarketPda,
    findOracleCachePda,
    findPositionPda,
    findTraderPda,
    findVenueGuardPda,
    GLOBAL_CONFIG_DISCRIMINATOR,
    getCloseShortInstructionAsync,
    getCrankPriceInstructionAsync,
    getCrankSolUsdInstructionAsync,
    getCreateMarketInstructionAsync,
    getDepositInstructionAsync,
    getGlobalConfigDecoder,
    getLenderPositionDecoder,
    getMarketDecoder,
    getOpenShortInstructionAsync,
    getOracleCacheDecoder,
    getShortPositionDecoder,
    getVenueExtrasDecoder,
    getVenueGuardDecoder,
    getWithdrawInstructionAsync,
    LENDER_POSITION_DISCRIMINATOR,
    MARKET_DISCRIMINATOR,
    ORACLE_CACHE_DISCRIMINATOR,
    SHORT_POSITION_DISCRIMINATOR,
    SHORTWIN_PROGRAM_ADDRESS,
    VENUE_EXTRAS_DISCRIMINATOR,
    VENUE_GUARD_DISCRIMINATOR,
} from '@shortwin/client';
import {
    address,
    appendTransactionMessageInstructions,
    compileTransaction,
    compressTransactionMessageUsingAddressLookupTables,
    createKeyPairSignerFromBytes,
    createSolanaRpc,
    createSolanaRpcSubscriptions,
    createTransactionMessage,
    fetchAddressesForLookupTables,
    getAddressEncoder,
    getBase64EncodedWireTransaction,
    getBase64Encoder,
    getProgramDerivedAddress,
    getSignatureFromTransaction,
    getTransactionSize,
    sendAndConfirmTransactionFactory,
    setTransactionMessageComputeUnitLimit,
    setTransactionMessageFeePayerSigner,
    setTransactionMessageLifetimeUsingBlockhash,
    signTransactionMessageWithSigners,
} from '@solana/kit';
import { getTransferSolInstruction } from '@solana-program/system';
import { findAssociatedTokenPda } from '@solana-program/token';
import {
    EXTRAS_IX,
    GUARD_MASK,
    PUMP_PROGRAM,
    SHORTWIN_PROGRAM,
    TOKEN_2022_PROGRAM,
    VENUE_BIT,
} from '../src/venues/constants.ts';
import { detectFrom } from '../src/venues/detect.ts';
import { extrasCandidates, keeperRecipientIndex, readMetas, readSet, tradeExtras } from '../src/venues/lists.ts';
import { bondingCurve, canonicalPool, poolAuthority, venueExtras } from '../src/venues/pda.ts';

export const DEVNET_GENESIS = 'EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG';
export const PROTOCOL_ALT = 'F9VQX1wvHjzthd3BaQQQ2d22WLJxogAbPRKMZEnCuNna';
export const FUND_ROLES = Object.freeze({
    keeper: { address: '35DeTJZZKgHzbLfWe9yiM1EwFfVUTcPCFSizLd8crDbc', ceiling: 2_000_000_000n },
    lender: { address: 'APCRy7175QKaNK7YM8QcvxTFd7npQQPjwfKpnzqRdfN2', ceiling: 1_000_000_000n },
    borrower: { address: '5HXuXQvF2iqDqT2VjCWwk2J6gNXg5CNeob4DdabaYqBZ', ceiling: 1_000_000_000n },
    fee_recipient: { address: 'HEjqYyDMp3UtT2sQWiqpNLd9t4w3svDXFDLfZ6bs3XSY', ceiling: 1_000_000n },
});
const DEPLOYMENT_PAYER = '3JfrUHa1RihL7rgJDWSmdV9DNGXkwKAUAcRDBh75cS14';
const CLOCK = address('SysvarC1ock11111111111111111111111111111111');
const SYSTEM_PROGRAM = address('11111111111111111111111111111111');
const UPGRADEABLE_LOADER = address('BPFLoaderUpgradeab1e11111111111111111111111');
const LOOKUP_TABLE_PROGRAM = address('AddressLookupTab1e1111111111111111111111111');
const PYTH_RECEIVER = address('rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ');
const U64_MAX = 18_446_744_073_709_551_615n;
const MAX_WIRE = 1232;
const MAX_TRADE_SPEND = 1_000_000_000n;
const MAX_FUND_SPEND = 2_000_100_000n;
const encoder = getBase64Encoder();

const HELP = [
    'usage: node scripts/devnet-trade-smoke.mjs --phase PHASE --keypair FILE [options] [--send]',
    '',
    'Phases: fund, create-market, deposit, crank-oracle, crank-price, open, close, withdraw.',
    'Every phase simulates at confirmed commitment before --send can sign and submit.',
    'All phases require --max-spend-lamports; fund also requires --role and --lamports.',
    'Trade phases require --mint; all signing keys must already exist outside Git.',
    'The verified protocol ALT is used for all non-funding v0 transactions.',
    '',
    'fund:          --role keeper|lender|borrower|fee_recipient --lamports N',
    'deposit:       --amount-raw N [--min-shares-raw N]',
    'open:          --q-raw N --min-proceeds-lamports N --max-margin-lamports N',
    '               --max-open-fee-lamports N [--nonce N]',
    'close:         --position ADDRESS --max-buyback-cost-lamports N',
    'withdraw:      [--shares-raw N] [--min-tokens-raw N] [--min-sol-lamports N]',
];

export function parseArgs(argv) {
    if (argv.includes('--help') || argv.includes('-h')) return { help: true };
    const out = { send: false };
    const allowed = new Set([
        'phase',
        'keypair',
        'mint',
        'rpc',
        'ws',
        'alt',
        'max-spend-lamports',
        'role',
        'lamports',
        'amount-raw',
        'min-shares-raw',
        'q-raw',
        'min-proceeds-lamports',
        'max-margin-lamports',
        'max-open-fee-lamports',
        'nonce',
        'position',
        'max-buyback-cost-lamports',
        'shares-raw',
        'min-tokens-raw',
        'min-sol-lamports',
    ]);
    for (let i = 0; i < argv.length; i++) {
        const arg = argv[i];
        if (arg === '--send') {
            if (out.send) throw new Error('duplicate --send');
            out.send = true;
            continue;
        }
        if (!arg?.startsWith('--')) throw new Error('unexpected argument ' + String(arg));
        const key = arg.slice(2);
        if (!allowed.has(key) || out[key] !== undefined) throw new Error('unknown or duplicate option ' + arg);
        const value = argv[++i];
        if (!value || value.startsWith('--')) throw new Error(arg + ' needs a value');
        out[key] = value;
    }
    if (
        !['fund', 'create-market', 'deposit', 'crank-oracle', 'crank-price', 'open', 'close', 'withdraw'].includes(
            out.phase,
        )
    ) {
        throw new Error('invalid --phase');
    }
    out.rpc ??= 'https://api.devnet.solana.com';
    out.ws ??= 'wss://api.devnet.solana.com';
    if (!out.keypair) throw new Error('--keypair is required');
    if (out.phase !== 'fund' && out.phase !== 'crank-oracle' && !out.mint) throw new Error('--mint is required');
    if (out.phase === 'fund' && (!FUND_ROLES[out.role] || !out.lamports))
        throw new Error('fund needs --role and --lamports');
    if (out.phase === 'deposit' && !out['amount-raw']) throw new Error('deposit needs --amount-raw');
    if (out.phase === 'open') {
        for (const k of ['q-raw', 'min-proceeds-lamports', 'max-margin-lamports', 'max-open-fee-lamports']) {
            if (out[k] === undefined) throw new Error('open needs --' + k);
        }
    }
    if (out.phase === 'close' && (!out.position || !out['max-buyback-cost-lamports'])) {
        throw new Error('close needs --position and --max-buyback-cost-lamports');
    }
    if (out['max-spend-lamports'] === undefined) throw new Error('--max-spend-lamports is required');
    const spend = uint(out['max-spend-lamports'], 'max-spend-lamports');
    if (spend === 0n || spend > (out.phase === 'fund' ? MAX_FUND_SPEND : MAX_TRADE_SPEND)) {
        throw new Error('max spend exceeds the devnet smoke ceiling');
    }
    if (out.send && out.phase === 'deposit' && uint(out['min-shares-raw'] ?? '0', 'min-shares-raw') === 0n) {
        throw new Error('sending deposit needs a positive --min-shares-raw');
    }
    if (out.send && out.phase === 'withdraw' && uint(out['min-tokens-raw'] ?? '0', 'min-tokens-raw') === 0n) {
        throw new Error('sending withdraw needs a positive --min-tokens-raw');
    }
    if (out.rpc.includes('mainnet') || out.ws.includes('mainnet')) throw new Error('mainnet endpoint refused');
    if (out.alt && out.alt !== PROTOCOL_ALT) throw new Error('only the reviewed protocol ALT is accepted');
    if (out.phase === 'create-market' && spend > 450_000_000n) {
        throw new Error('market creation spend cap is 0.45 SOL within the 5-SOL payer plan');
    }
    return out;
}

export function uint(s, name) {
    if (typeof s !== 'string' || !/^\d{1,20}$/.test(s)) throw new Error(name + ' must be an unsigned decimal integer');
    const n = BigInt(s);
    if (n > U64_MAX) throw new Error(name + ' exceeds u64');
    return n;
}

function needPositive(s, name) {
    const n = uint(s, name);
    if (n === 0n) throw new Error(name + ' must be positive');
    return n;
}

async function loadSigner(file) {
    const mode = statSync(file).mode & 0o777;
    if ((mode & 0o077) !== 0) throw new Error('keypair file must not be readable by group or others');
    const bytes = JSON.parse(readFileSync(file, 'utf8'));
    if (!Array.isArray(bytes) || bytes.length !== 64 || bytes.some((x) => !Number.isInteger(x) || x < 0 || x > 255)) {
        throw new Error('keypair file is not a 64-byte Solana CLI JSON keypair');
    }
    return createKeyPairSignerFromBytes(new Uint8Array(bytes));
}

function sameBytes(a, b) {
    return a.length === b.length && a.every((x, i) => x === b[i]);
}

function decodeOwned(raw, owner, discriminator, decoder, label) {
    if (!raw || raw.owner !== owner || !sameBytes(raw.data.subarray(0, 8), discriminator)) {
        throw new Error(label + ' missing, foreign-owned or wrong discriminator');
    }
    return decoder.decode(raw.data);
}

async function getRaw(rpc, key) {
    const { value } = await rpc.getAccountInfo(key, { encoding: 'base64', commitment: 'confirmed' }).send();
    if (!value) return null;
    return {
        address: key,
        owner: value.owner,
        lamports: BigInt(value.lamports),
        executable: value.executable,
        data: new Uint8Array(encoder.encode(value.data[0])),
    };
}

async function getRaws(rpc, keys) {
    if (keys.length === 0) return [];
    const { value } = await rpc.getMultipleAccounts(keys, { encoding: 'base64', commitment: 'confirmed' }).send();
    if (value.length !== keys.length) throw new Error('RPC returned an incomplete account batch');
    return value.map((account, i) =>
        account
            ? {
                  address: keys[i],
                  owner: account.owner,
                  lamports: BigInt(account.lamports),
                  executable: account.executable,
                  data: new Uint8Array(encoder.encode(account.data[0])),
              }
            : null,
    );
}

function decodeClock(raw) {
    if (!raw || raw.data.length < 40) throw new Error('Clock sysvar unavailable');
    const d = new DataView(raw.data.buffer, raw.data.byteOffset);
    return { slot: d.getBigUint64(0, true), unix: d.getBigInt64(32, true) };
}

async function eventAuthority() {
    const [key] = await getProgramDerivedAddress({
        programAddress: SHORTWIN_PROGRAM_ADDRESS,
        seeds: ['__event_authority'],
    });
    return key;
}

async function preflight(rpc, opts) {
    const genesis = await rpc.getGenesisHash().send();
    if (genesis !== DEVNET_GENESIS) throw new Error('refusing non-devnet genesis ' + genesis);
    if (opts.phase === 'fund') {
        const [program, clockRaw] = await getRaws(rpc, [SHORTWIN_PROGRAM_ADDRESS, CLOCK]);
        if (!program?.executable || program.owner !== UPGRADEABLE_LOADER)
            throw new Error('deployed ShortWin program is unavailable');
        return { now: decodeClock(clockRaw) };
    }
    const [configKey] = await findConfigPda();
    const [guardKey] = await findVenueGuardPda();
    const [oracleKey] = await findOracleCachePda();
    const mint = opts.phase === 'crank-oracle' ? null : address(opts.mint);
    const marketKey = mint ? (await findMarketPda({ mint }))[0] : null;
    const curveKey = mint ? await bondingCurve(mint) : null;
    const authority = mint ? await poolAuthority(mint) : null;
    const poolKey = mint ? await canonicalPool(mint, authority) : null;
    const coreKeys = [SHORTWIN_PROGRAM_ADDRESS, configKey, guardKey, oracleKey, CLOCK];
    const keys = mint ? [...coreKeys, mint, marketKey, curveKey] : coreKeys;
    const [program, configRaw, guardRaw, oracleRaw, clockRaw, mintRaw, marketRaw, curve] = await getRaws(rpc, keys);
    if (!program?.executable || program.owner !== UPGRADEABLE_LOADER)
        throw new Error('deployed ShortWin program is unavailable');
    const config = decodeOwned(
        configRaw,
        SHORTWIN_PROGRAM,
        GLOBAL_CONFIG_DISCRIMINATOR,
        getGlobalConfigDecoder(),
        'GlobalConfig',
    );
    const guard = decodeOwned(
        guardRaw,
        SHORTWIN_PROGRAM,
        VENUE_GUARD_DISCRIMINATOR,
        getVenueGuardDecoder(),
        'VenueGuard',
    );
    const oracle = oracleRaw
        ? decodeOwned(oracleRaw, SHORTWIN_PROGRAM, ORACLE_CACHE_DISCRIMINATOR, getOracleCacheDecoder(), 'OracleCache')
        : null;
    const now = decodeClock(clockRaw);
    if (opts.phase === 'crank-oracle') return { config, guard, oracle, now };
    if (!mintRaw || mintRaw.owner !== TOKEN_2022_PROGRAM)
        throw new Error('controlled Pump mint must be a Token-2022 account');
    const market = marketRaw
        ? decodeOwned(marketRaw, SHORTWIN_PROGRAM, MARKET_DISCRIMINATOR, getMarketDecoder(), 'Market')
        : null;
    if (market && (market.mint !== mint || market.tokenProgram !== TOKEN_2022_PROGRAM)) {
        throw new Error('market mint or token program does not match the selected mint');
    }
    if (opts.phase === 'create-market' && market) throw new Error('market already exists');
    if (opts.phase !== 'create-market' && !market) throw new Error('market is absent; create it first');
    if ((config.venueEnabled & (VENUE_BIT.pump_curve ?? 0)) === 0) throw new Error('Pump curve venue is disabled');
    if ((guard.upgraded & (GUARD_MASK.pump_curve ?? 0)) !== 0) throw new Error('Pump venue guard is flagged');
    if (market && market.venue !== 1) throw new Error('this smoke accepts only a live Pump curve market');
    if (!curve || curve.owner !== PUMP_PROGRAM) throw new Error('Pump bonding curve missing');
    if (market && (market.tier !== 3 || market.bondingCurve !== curveKey || market.canonicalPool !== poolKey)) {
        throw new Error('market is not bound to the expected Tier D Pump curve');
    }
    return { config, guard, oracle, now, mint, mintRaw, marketKey, market, curveKey, poolKey, curve };
}

async function controlledCurve(rpc, state, signer) {
    const detected = await detectFrom(
        state.mint,
        { bondingCurve: state.curveKey, canonicalPool: state.poolKey, authority: await poolAuthority(state.mint) },
        state.curve,
        await getRaw(rpc, state.poolKey),
        signer,
    );
    if (detected.refusal || detected.kind !== 'pump_curve') {
        throw new Error(
            'the mint is not a supported live SOL Pump curve: ' + String(detected.refusal ?? detected.kind),
        );
    }
    return detected;
}

async function marketReadSet(rpc, state) {
    if (!state.market) throw new Error('market missing');
    const keys = await readSet('pump_curve', state.market);
    const read = await getRaws(rpc, keys);
    return { keys, read };
}

function sourceToken(raw, mint, wallet, amount) {
    if (!raw || raw.owner !== TOKEN_2022_PROGRAM || raw.data.length < 72)
        throw new Error('lender token account missing');
    const addrBytes = new Uint8Array(raw.data.subarray(0, 32));
    const walletBytes = new Uint8Array(raw.data.subarray(32, 64));
    const encode = getAddressEncoder();
    if (!sameBytes(addrBytes, encode.encode(mint)) || !sameBytes(walletBytes, encode.encode(wallet))) {
        throw new Error('lender token account mint or owner mismatch');
    }
    const balance = new DataView(raw.data.buffer, raw.data.byteOffset).getBigUint64(64, true);
    if (balance < amount) throw new Error('lender token balance below deposit amount');
    return balance;
}

async function buildPhase(rpc, opts, signer, state) {
    const phase = opts.phase;
    const requiredSigner =
        phase === 'fund' || phase === 'create-market'
            ? DEPLOYMENT_PAYER
            : phase === 'deposit' || phase === 'withdraw'
              ? FUND_ROLES.lender.address
              : phase === 'open' || phase === 'close'
                ? FUND_ROLES.borrower.address
                : FUND_ROLES.keeper.address;
    if (signer.address !== requiredSigner) throw new Error('signer does not match the staged role for ' + phase);
    if (phase === 'fund') {
        const role = FUND_ROLES[opts.role];
        const amount = needPositive(opts.lamports, 'lamports');
        const recipient = address(role.address);
        const prior = await rpc.getBalance(recipient, { commitment: 'confirmed' }).send();
        const balance = BigInt(prior.value);
        const account = await getRaw(rpc, recipient);
        if (account && account.owner !== SYSTEM_PROGRAM) throw new Error('funding recipient is not system-owned');
        if (balance !== 0n) throw new Error('recipient is already funded; fund phase is one-time per role');
        if (amount > role.ceiling) throw new Error('funding would exceed the role target');
        if (amount > uint(opts['max-spend-lamports'], 'max-spend-lamports')) {
            throw new Error('fund amount exceeds this transaction spend cap');
        }
        return {
            instructions: [getTransferSolInstruction({ source: signer, destination: recipient, amount })],
            watch: [recipient],
            summary: {
                phase,
                recipientRole: opts.role,
                recipient,
                amountLamports: amount.toString(),
                balanceBefore: balance.toString(),
            },
        };
    }
    if (phase === 'crank-oracle') {
        const price = await getRaw(rpc, state.config.pythSolUsd);
        if (!price || price.owner !== PYTH_RECEIVER)
            throw new Error('configured Pyth price-update account missing or foreign-owned');
        return {
            instructions: [await getCrankSolUsdInstructionAsync({ priceUpdate: state.config.pythSolUsd })],
            watch: [(await findOracleCachePda())[0]],
            summary: {
                phase,
                priceUpdate: state.config.pythSolUsd,
                oraclePublishTime: state.oracle?.publishTime?.toString() ?? '0',
            },
        };
    }
    const mint = state.mint;
    const event = await eventAuthority();
    if (phase === 'create-market') {
        const detected = await controlledCurve(rpc, state, signer);
        const [trader] = await findTraderPda({ market: state.marketKey });
        const [traderBaseAta] = await findAssociatedTokenPda({ owner: trader, mint, tokenProgram: TOKEN_2022_PROGRAM });
        const creationFee = state.config.params.marketCreationFee;
        if (creationFee > 20_000_000n) throw new Error('market creation fee above the devnet smoke ceiling');
        const ix = await getCreateMarketInstructionAsync({
            creator: signer,
            mint,
            traderBaseAta,
            tokenProgram: TOKEN_2022_PROGRAM,
            eventAuthority: event,
            program: SHORTWIN_PROGRAM_ADDRESS,
            venueHint: 0,
            maxCreationFee: creationFee,
        });
        return {
            instructions: [
                ...detected.preInstructions,
                { ...ix, accounts: [...ix.accounts, ...detected.remainingAccounts] },
            ],
            watch: [state.marketKey],
            summary: {
                phase,
                mint,
                market: state.marketKey,
                venue: detected.kind,
                maxCreationFeeLamports: creationFee.toString(),
            },
        };
    }
    if (phase === 'deposit') {
        const amount = needPositive(opts['amount-raw'], 'amount-raw');
        const minShares = uint(opts['min-shares-raw'] ?? '0', 'min-shares-raw');
        const [source] = await findAssociatedTokenPda({
            owner: signer.address,
            mint,
            tokenProgram: TOKEN_2022_PROGRAM,
        });
        const sourceRaw = await getRaw(rpc, source);
        sourceToken(sourceRaw, mint, signer.address, amount);
        const [lender] = await findLenderPda({ market: state.marketKey, owner: signer.address });
        const ix = await getDepositInstructionAsync({
            owner: signer,
            mint,
            ownerTokenAccount: source,
            tokenProgram: TOKEN_2022_PROGRAM,
            eventAuthority: event,
            program: SHORTWIN_PROGRAM_ADDRESS,
            amount,
            minShares,
        });
        return {
            instructions: [ix],
            watch: [source, lender],
            summary: {
                phase,
                mint,
                market: state.marketKey,
                lender,
                source,
                amountRaw: amount.toString(),
                minSharesRaw: minShares.toString(),
            },
        };
    }
    if (phase === 'crank-price') {
        const set = await marketReadSet(rpc, state);
        const ix = await getCrankPriceInstructionAsync({
            mint,
            eventAuthority: event,
            program: SHORTWIN_PROGRAM_ADDRESS,
        });
        return {
            instructions: [{ ...ix, accounts: [...ix.accounts, ...readMetas('pump_curve', set.keys, false)] }],
            watch: [state.marketKey],
            summary: { phase, mint, market: state.marketKey, clock: state.now.unix.toString() },
        };
    }
    if (phase === 'open' || phase === 'close') {
        const set = await marketReadSet(rpc, state);
        const ixName = phase === 'open' ? EXTRAS_IX.sell : EXTRAS_IX.buy;
        const extrasAddress = await venueExtras(1);
        const extrasRaw = await getRaw(rpc, extrasAddress);
        const extrasState = decodeOwned(
            extrasRaw,
            SHORTWIN_PROGRAM,
            VENUE_EXTRAS_DISCRIMINATOR,
            getVenueExtrasDecoder(),
            'Pump VenueExtras',
        );
        const configured = extrasState.sets[ixName];
        if (configured?.current.count !== 0 || configured.previous.count !== 0 || configured.stable.count !== 0)
            throw new Error('configured venue extras need a reviewed transaction plan');
        const extras = await extrasCandidates('pump_curve', ixName, state.market, extrasState);
        if (extras.length !== 1 || extras[0].label !== 'none') throw new Error('unreviewed venue extras');
        const remaining = [
            ...readMetas('pump_curve', set.keys, true),
            ...(await tradeExtras('pump_curve', state.market, set.read, keeperRecipientIndex)),
        ];
        if (phase === 'open') {
            if (state.now.unix - state.market.createdAt < BigInt(state.config.params.marketWarmupS)) {
                throw new Error('market warm-up has not elapsed');
            }
            if (
                !state.oracle ||
                state.oracle.publishTime <= 0n ||
                state.now.unix - state.oracle.publishTime > BigInt(state.config.params.oracleMaxAgeS)
            ) {
                throw new Error('SOL/USD oracle cache is missing or stale');
            }
            const q = needPositive(opts['q-raw'], 'q-raw');
            const minProceeds = uint(opts['min-proceeds-lamports'], 'min-proceeds-lamports');
            const maxMargin = needPositive(opts['max-margin-lamports'], 'max-margin-lamports');
            const maxOpenFee = uint(opts['max-open-fee-lamports'], 'max-open-fee-lamports');
            if (maxMargin > 500_000_000n || maxOpenFee > 100_000_000n)
                throw new Error('open bounds exceed tiny-smoke ceilings');
            const nonce = opts.nonce ? uint(opts.nonce, 'nonce') : randomBytes(8).readBigUInt64LE();
            const [position] = await findPositionPda({ market: state.marketKey, borrower: signer.address, nonce });
            if (await getRaw(rpc, position)) throw new Error('position nonce already used');
            const ix = await getOpenShortInstructionAsync({
                borrower: signer,
                mint,
                trader: state.market.trader,
                tokenProgram: TOKEN_2022_PROGRAM,
                eventAuthority: event,
                program: SHORTWIN_PROGRAM_ADDRESS,
                nonce,
                q,
                minProceeds,
                maxMargin,
                maxOpenFee,
                extras: { set: 0, bumps: new Uint8Array(4) },
            });
            return {
                instructions: [{ ...ix, accounts: [...ix.accounts, ...remaining] }],
                watch: [position, state.marketKey],
                summary: {
                    phase,
                    mint,
                    market: state.marketKey,
                    position,
                    borrower: signer.address,
                    nonce: nonce.toString(),
                    qRaw: q.toString(),
                    minProceedsLamports: minProceeds.toString(),
                    maxMarginLamports: maxMargin.toString(),
                    maxOpenFeeLamports: maxOpenFee.toString(),
                },
            };
        }
        const position = address(opts.position);
        const pos = decodeOwned(
            await getRaw(rpc, position),
            SHORTWIN_PROGRAM,
            SHORT_POSITION_DISCRIMINATOR,
            getShortPositionDecoder(),
            'ShortPosition',
        );
        if (pos.market !== state.marketKey || pos.borrower !== signer.address) {
            throw new Error('position does not belong to selected market and borrower');
        }
        const maxBuybackCost = needPositive(opts['max-buyback-cost-lamports'], 'max-buyback-cost-lamports');
        if (maxBuybackCost > 1_000_000_000n) throw new Error('close buyback cap exceeds tiny-smoke ceiling');
        const ix = await getCloseShortInstructionAsync({
            borrower: signer,
            position,
            mint,
            trader: state.market.trader,
            tokenProgram: TOKEN_2022_PROGRAM,
            eventAuthority: event,
            program: SHORTWIN_PROGRAM_ADDRESS,
            maxBuybackCost,
            extras: { set: 0, bumps: new Uint8Array(4) },
        });
        return {
            instructions: [{ ...ix, accounts: [...ix.accounts, ...remaining] }],
            watch: [position, state.marketKey],
            summary: {
                phase,
                mint,
                market: state.marketKey,
                position,
                maxBuybackCostLamports: maxBuybackCost.toString(),
            },
        };
    }
    if (phase === 'withdraw') {
        const [lender] = await findLenderPda({ market: state.marketKey, owner: signer.address });
        const l = decodeOwned(
            await getRaw(rpc, lender),
            SHORTWIN_PROGRAM,
            LENDER_POSITION_DISCRIMINATOR,
            getLenderPositionDecoder(),
            'LenderPosition',
        );
        if (l.market !== state.marketKey || l.owner !== signer.address) throw new Error('lender position mismatch');
        if (l.epoch !== state.market.shareEpoch || l.fwd !== 0)
            throw new Error('lender position needs epoch synchronization');
        if (l.lockUntil > state.now.unix)
            throw new Error('lender position remains locked until ' + l.lockUntil.toString());
        const shares = opts['shares-raw'] ? needPositive(opts['shares-raw'], 'shares-raw') : l.shares;
        if (shares === 0n || shares > l.shares) throw new Error('withdraw shares exceed current position');
        const minTokens = uint(opts['min-tokens-raw'] ?? '0', 'min-tokens-raw');
        const minSol = uint(opts['min-sol-lamports'] ?? '0', 'min-sol-lamports');
        const [destination] = await findAssociatedTokenPda({
            owner: signer.address,
            mint,
            tokenProgram: TOKEN_2022_PROGRAM,
        });
        const ix = await getWithdrawInstructionAsync({
            owner: signer,
            mint,
            ownerTokenAccount: destination,
            tokenProgram: TOKEN_2022_PROGRAM,
            eventAuthority: event,
            program: SHORTWIN_PROGRAM_ADDRESS,
            shares,
            minTokens,
            minSol,
        });
        return {
            instructions: [ix],
            watch: [lender, destination],
            summary: {
                phase,
                mint,
                market: state.marketKey,
                lender,
                destination,
                sharesRaw: shares.toString(),
                minTokensRaw: minTokens.toString(),
                minSolLamports: minSol.toString(),
            },
        };
    }
    throw new Error('unsupported phase ' + phase);
}

async function lookupTables(rpc, phase) {
    if (phase === 'fund') return {};
    const key = address(PROTOCOL_ALT);
    const raw = await getRaw(rpc, key);
    if (!raw || raw.owner !== LOOKUP_TABLE_PROGRAM || raw.data.length < 56 + 97 * 32) {
        throw new Error('reviewed protocol ALT is missing or shorter than the verified 97 entries');
    }
    const tables = await fetchAddressesForLookupTables([key], rpc);
    if (!tables[key] || tables[key].length < 97) throw new Error('protocol ALT lookup failed');
    return tables;
}

function instructionMessage(signer, lifetime, instructions, tables) {
    const start = createTransactionMessage({ version: 0 });
    const payer = setTransactionMessageFeePayerSigner(signer, start);
    const timed = setTransactionMessageLifetimeUsingBlockhash(lifetime, payer);
    const metered = setTransactionMessageComputeUnitLimit(1_400_000, timed);
    const assembled = appendTransactionMessageInstructions(instructions, metered);
    return Object.keys(tables).length
        ? compressTransactionMessageUsingAddressLookupTables(assembled, tables)
        : assembled;
}

async function simulatePhase(rpc, signer, opts, plan) {
    const { value: lifetime } = await rpc.getLatestBlockhash({ commitment: 'confirmed' }).send();
    const tables = await lookupTables(rpc, opts.phase);
    const message = instructionMessage(signer, lifetime, plan.instructions, tables);
    const transaction = compileTransaction(message);
    const size = getTransactionSize(transaction);
    if (size > MAX_WIRE) throw new Error('transaction exceeds the 1232-byte packet limit: ' + size);
    const before = await rpc.getBalance(signer.address, { commitment: 'confirmed' }).send();
    const watched = [...new Set([signer.address, ...plan.watch])];
    const simulation = await rpc
        .simulateTransaction(getBase64EncodedWireTransaction(transaction), {
            encoding: 'base64',
            sigVerify: false,
            replaceRecentBlockhash: true,
            commitment: 'confirmed',
            accounts: { addresses: watched, encoding: 'base64' },
        })
        .send();
    const result = simulation.value;
    if (result.err !== null) {
        throw new Error(
            'simulation failed: ' +
                JSON.stringify(result.err, (_, value) => (typeof value === 'bigint' ? value.toString() : value)) +
                '\n' +
                (result.logs ?? []).slice(-20).join('\n'),
        );
    }
    const simulatedPayer = result.accounts?.[0];
    if (!simulatedPayer) throw new Error('simulation omitted fee-payer account; spend cannot be bounded');
    const beforeLamports = BigInt(before.value);
    const afterLamports = BigInt(simulatedPayer.lamports);
    const spend = beforeLamports > afterLamports ? beforeLamports - afterLamports : 0n;
    // The simulation may omit the signature fee. Reserve 50,000 lamports beyond its state delta.
    const feeAllowance = 50_000n;
    const ceiling = uint(opts['max-spend-lamports'], 'max-spend-lamports');
    if (spend + feeAllowance > ceiling) {
        throw new Error('simulated payer debit plus fee allowance exceeds --max-spend-lamports');
    }
    const minimumReserve = signer.address === DEPLOYMENT_PAYER ? 5_000_000_000n : 10_000_000n;
    if (afterLamports < minimumReserve + feeAllowance) {
        throw new Error('simulated payer balance would fall below its required reserve');
    }
    const report = {
        ...plan.summary,
        mode: opts.send ? 'send after successful simulation' : 'simulation only',
        program: opts.phase === 'fund' ? SYSTEM_PROGRAM : SHORTWIN_PROGRAM_ADDRESS,
        genesis: DEVNET_GENESIS,
        lookupTable: opts.phase === 'fund' ? null : PROTOCOL_ALT,
        packetBytes: size,
        computeUnits: result.unitsConsumed?.toString() ?? null,
        payer: signer.address,
        payerBeforeLamports: beforeLamports.toString(),
        payerAfterSimulationLamports: afterLamports.toString(),
        simulatedDebitLamports: spend.toString(),
        feeAllowanceLamports: feeAllowance.toString(),
        maxSpendLamports: ceiling.toString(),
    };
    return { message, report };
}

export async function main(argv = process.argv.slice(2)) {
    const opts = parseArgs(argv);
    if (opts.help) {
        console.log(HELP.join('\n'));
        return;
    }
    const signer = await loadSigner(opts.keypair);
    const rpc = createSolanaRpc(opts.rpc);
    const state = await preflight(rpc, opts);
    const plan = await buildPhase(rpc, opts, signer, state);
    const { message, report } = await simulatePhase(rpc, signer, opts, plan);
    console.log(JSON.stringify(report, null, 2));
    if (!opts.send) return;
    const signed = await signTransactionMessageWithSigners(message);
    const signature = getSignatureFromTransaction(signed);
    const sendAndConfirm = sendAndConfirmTransactionFactory({
        rpc,
        rpcSubscriptions: createSolanaRpcSubscriptions(opts.ws),
    });
    try {
        await sendAndConfirm(signed, { commitment: 'confirmed' });
        console.log(JSON.stringify({ phase: opts.phase, status: 'confirmed', signature }));
    } catch (error) {
        throw new Error(
            'send or confirmation failed; check the signature before retrying: ' +
                signature +
                ' (' +
                String(error?.message ?? error) +
                ')',
        );
    }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
    main().catch((error) => {
        console.error('devnet trade smoke: ' + String(error?.message ?? error));
        process.exitCode = 1;
    });
}

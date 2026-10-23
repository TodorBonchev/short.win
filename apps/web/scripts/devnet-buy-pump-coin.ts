/**
 * Buy lender inventory on a controlled SOL-paired Pump curve on devnet.
 *
 * Pump `buy_v3` account order and discriminator: pump-fun/pump-public-docs commit
 * 639b87fb45c3adeffa6f7c4a1bf0cb85e392bf22, idl/pump.json SHA-256
 * 7616681ea557975f9f623ac211080f22a93060781eaf4765b0a488d980997207.
 * https://github.com/pump-fun/pump-public-docs/blob/639b87fb45c3adeffa6f7c4a1bf0cb85e392bf22/docs/instructions/TRADE_V3.md
 *
 * The buyer's Token-2022 ATA is created idempotently in the same packet. A dry-run
 * never signs or sends. `--send` requires a successful unsigned simulation and
 * checks the total payer debit, including ATA rent and fees, against the cap.
 * Keep the buyer keypair outside Git in mode 0600.
 */

import { readFileSync, statSync } from 'node:fs';
import {
    type AccountMeta,
    AccountRole,
    type Address,
    appendTransactionMessageInstructions,
    compileTransaction,
    createKeyPairSignerFromBytes,
    createSolanaRpc,
    createTransactionMessage,
    getAddressDecoder,
    getAddressEncoder,
    getBase64EncodedWireTransaction,
    getProgramDerivedAddress,
    getSignatureFromTransaction,
    getTransactionSize,
    getUtf8Encoder,
    setTransactionMessageFeePayerSigner,
    setTransactionMessageLifetimeUsingBlockhash,
    signTransactionMessageWithSigners,
} from '@solana/kit';
import {
    findAssociatedTokenPda,
    getCreateAssociatedTokenIdempotentInstructionAsync,
    TOKEN_2022_PROGRAM_ADDRESS,
} from '@solana-program/token-2022';

const RPC_URL = 'https://api.devnet.solana.com';
const DEVNET_GENESIS = 'EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG';
const PUMP = '6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P' as Address;
const PUMP_FEES = 'pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ' as Address;
const GLOBAL = '4wTV1YmiEkRvAtNtsSGPtUrqRYQMe5SKy2uB4Jjaxnjf' as Address;
const FEE_CONFIG = '8Wf5TiAheLUqBrKXeYg2JtAFFMWtKdG2BSFgqUcPVwTt' as Address;
const EVENT_AUTHORITY = 'Ce6TQqeHC9p8KetsN6JsjHK7UTZk7nasjjnr7XxXp9F1' as Address;
const SYSTEM = '11111111111111111111111111111111' as Address;
const TOKEN_PROGRAM = 'TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA' as Address;
const WSOL = 'So11111111111111111111111111111111111111112' as Address;
const UPGRADEABLE_LOADER = 'BPFLoaderUpgradeab1e11111111111111111111111' as Address;
const EXPECTED_PUMP_DEPLOY_SLOT = 510_657_843n;
const EXPECTED_PUMP_FEES_DEPLOY_SLOT = 508_659_783n;
const BUY_V3_DISCRIMINATOR = Uint8Array.of(7, 5, 29, 196, 245, 23, 101, 80);
const GLOBAL_DISCRIMINATOR = Uint8Array.of(0xa7, 0xe8, 0xe8, 0xb1, 0xc8, 0x6c, 0x72, 0x7f);
const CURVE_DISCRIMINATOR = Uint8Array.of(0x17, 0xb7, 0xf8, 0x37, 0x60, 0xd8, 0xac, 0x60);
const OFF_GLOBAL_BUYBACK_FEE_RECIPIENTS = 741;
const OFF_CURVE_REAL_TOKEN_RESERVES = 24;
const OFF_CURVE_COMPLETE = 48;
const OFF_CURVE_CREATOR = 49;
const OFF_CURVE_MAYHEM = 81;
const OFF_CURVE_CASHBACK = 82;
const OFF_CURVE_QUOTE_MINT = 83;

type Options = {
    buyerKeypair: string;
    mint: Address;
    expectedCreator: Address;
    tokenAmountRaw: bigint;
    maxSolCostLamports: bigint;
    maxPayerDebitLamports: bigint;
    send: boolean;
};

function parseArgs(argv: readonly string[]): Options {
    const values = new Map<string, string>();
    let send = false;
    const accepted = [
        '--buyer-keypair',
        '--mint',
        '--expected-creator',
        '--token-amount-raw',
        '--max-sol-cost-lamports',
        '--max-payer-debit-lamports',
    ];
    for (let i = 0; i < argv.length; i += 1) {
        const flag = argv[i];
        if (flag === '--send') {
            if (send) throw new Error('duplicate --send');
            send = true;
            continue;
        }
        if (!flag || !accepted.includes(flag)) throw new Error(`unknown option ${flag ?? '(empty)'}`);
        if (values.has(flag)) throw new Error(`duplicate ${flag}`);
        const value = argv[++i];
        if (!value || value.startsWith('--')) throw new Error(`${flag} needs a value`);
        values.set(flag, value);
    }
    const needed = (key: string): string => {
        const value = values.get(key);
        if (!value) throw new Error(`${key} is required`);
        return value;
    };
    const positiveU64 = (key: string): bigint => {
        const value = needed(key);
        if (!/^[1-9][0-9]*$/.test(value)) throw new Error(`${key} must be a positive integer`);
        const n = BigInt(value);
        if (n > 0xffff_ffff_ffff_ffffn) throw new Error(`${key} exceeds u64`);
        return n;
    };
    const mint = needed('--mint') as Address;
    const expectedCreator = needed('--expected-creator') as Address;
    getAddressEncoder().encode(mint);
    getAddressEncoder().encode(expectedCreator);
    const tokenAmountRaw = positiveU64('--token-amount-raw');
    const maxSolCostLamports = positiveU64('--max-sol-cost-lamports');
    const maxPayerDebitLamports = positiveU64('--max-payer-debit-lamports');
    if (maxSolCostLamports > 400_000_000n || maxPayerDebitLamports > 500_000_000n) {
        throw new Error('this devnet smoke caps the buy at 0.4 SOL and total debit at 0.5 SOL');
    }
    if (maxPayerDebitLamports <= maxSolCostLamports) {
        throw new Error('total debit cap must allow room above the trade cap for ATA rent and fees');
    }
    return {
        buyerKeypair: needed('--buyer-keypair'),
        mint,
        expectedCreator,
        tokenAmountRaw,
        maxSolCostLamports,
        maxPayerDebitLamports,
        send,
    };
}

async function signerFromFile(path: string) {
    if ((statSync(path).mode & 0o077) !== 0) throw new Error('keypair file must not be group/world accessible');
    const raw: unknown = JSON.parse(readFileSync(path, 'utf8'));
    if (!Array.isArray(raw) || raw.length !== 64 || !raw.every((n) => Number.isInteger(n) && n >= 0 && n <= 255)) {
        throw new Error('expected a Solana 64-byte JSON keypair file');
    }
    return createKeyPairSignerFromBytes(Uint8Array.from(raw));
}

function buyV3Data(amount: bigint, maxSolCost: bigint): Uint8Array {
    const out = new Uint8Array(24);
    out.set(BUY_V3_DISCRIMINATOR);
    const view = new DataView(out.buffer);
    view.setBigUint64(8, amount, true);
    view.setBigUint64(16, maxSolCost, true);
    // Omitted trailing partial_fill OptionBool decodes false for this non-Mayhem coin.
    return out;
}

function meta(address: Address, writable: boolean): AccountMeta {
    return { address, role: writable ? AccountRole.WRITABLE : AccountRole.READONLY };
}

function bytes(account: { data: readonly [string, string] }): Buffer {
    return Buffer.from(account.data[0], 'base64');
}

async function main(): Promise<void> {
    const opts = parseArgs(process.argv.slice(2));
    const buyer = await signerFromFile(opts.buyerKeypair);
    const rpc = createSolanaRpc(RPC_URL);
    const genesis = await rpc.getGenesisHash().send();
    if (genesis !== DEVNET_GENESIS) throw new Error(`wrong cluster genesis ${genesis}`);
    const utf8 = getUtf8Encoder();
    const [curve] = await getProgramDerivedAddress({
        programAddress: PUMP,
        seeds: [utf8.encode('bonding-curve'), getAddressEncoder().encode(opts.mint)],
    });
    const [baseCurveAta] = await findAssociatedTokenPda({
        owner: curve,
        mint: opts.mint,
        tokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
    });
    const [quoteCurveAta] = await findAssociatedTokenPda({ owner: curve, mint: WSOL, tokenProgram: TOKEN_PROGRAM });
    const [baseBuyerAta] = await findAssociatedTokenPda({
        owner: buyer.address,
        mint: opts.mint,
        tokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
    });
    const [quoteBuyerAta] = await findAssociatedTokenPda({
        owner: buyer.address,
        mint: WSOL,
        tokenProgram: TOKEN_PROGRAM,
    });
    const [userVolumeAccumulator] = await getProgramDerivedAddress({
        programAddress: PUMP,
        seeds: [utf8.encode('user_volume_accumulator'), getAddressEncoder().encode(buyer.address)],
    });
    const [programData] = await getProgramDerivedAddress({
        programAddress: UPGRADEABLE_LOADER,
        seeds: [getAddressEncoder().encode(PUMP)],
    });
    const [feesProgramData] = await getProgramDerivedAddress({
        programAddress: UPGRADEABLE_LOADER,
        seeds: [getAddressEncoder().encode(PUMP_FEES)],
    });
    const checked = await rpc
        .getMultipleAccounts([PUMP, GLOBAL, opts.mint, curve, baseCurveAta, FEE_CONFIG, programData, feesProgramData], {
            encoding: 'base64',
            commitment: 'confirmed',
        })
        .send();
    const [
        pumpProgram,
        globalAccount,
        mintAccount,
        curveAccount,
        curveAtaAccount,
        feeConfigAccount,
        programDataAccount,
        feesProgramDataAccount,
    ] = checked.value;
    if (
        !pumpProgram?.executable ||
        globalAccount?.owner !== PUMP ||
        mintAccount?.owner !== TOKEN_2022_PROGRAM_ADDRESS
    ) {
        throw new Error('Pump executable, Global or Token-2022 mint missing');
    }
    if (curveAccount?.owner !== PUMP || curveAtaAccount?.owner !== TOKEN_2022_PROGRAM_ADDRESS) {
        throw new Error('Pump curve or its Token-2022 ATA missing');
    }
    if (
        feeConfigAccount?.owner !== PUMP_FEES ||
        programDataAccount?.owner !== UPGRADEABLE_LOADER ||
        feesProgramDataAccount?.owner !== UPGRADEABLE_LOADER
    ) {
        throw new Error('Pump fee config or ProgramData owner mismatch');
    }
    const pd = bytes(programDataAccount);
    if (pd.length < 45 || pd.readUInt32LE(0) !== 3 || pd.readBigUInt64LE(4) !== EXPECTED_PUMP_DEPLOY_SLOT) {
        throw new Error('Pump devnet deployment changed; revalidate BuyV3 before proceeding');
    }
    const feesPd = bytes(feesProgramDataAccount);
    if (
        feesPd.length < 45 ||
        feesPd.readUInt32LE(0) !== 3 ||
        feesPd.readBigUInt64LE(4) !== EXPECTED_PUMP_FEES_DEPLOY_SLOT
    ) {
        throw new Error('Pump-fees devnet deployment changed; revalidate BuyV3 before proceeding');
    }
    const global = bytes(globalAccount);
    const curveData = bytes(curveAccount);
    if (global.length < 997 || !global.subarray(0, 8).equals(Buffer.from(GLOBAL_DISCRIMINATOR))) {
        throw new Error('Pump Global layout mismatch');
    }
    if (curveData.length < 115 || !curveData.subarray(0, 8).equals(Buffer.from(CURVE_DISCRIMINATOR))) {
        throw new Error('Pump curve layout mismatch');
    }
    const creator = getAddressDecoder().decode(curveData.subarray(OFF_CURVE_CREATOR, OFF_CURVE_CREATOR + 32));
    if (creator !== opts.expectedCreator) throw new Error('curve creator differs from expected controlled creator');
    if (curveData[OFF_CURVE_COMPLETE] || curveData[OFF_CURVE_MAYHEM] || curveData[OFF_CURVE_CASHBACK]) {
        throw new Error('coin must be incomplete, non-Mayhem and non-cashback');
    }
    if (!curveData.subarray(OFF_CURVE_QUOTE_MINT, OFF_CURVE_QUOTE_MINT + 32).every((x) => x === 0)) {
        throw new Error('coin is not SOL-quoted');
    }
    const realReserves = curveData.readBigUInt64LE(OFF_CURVE_REAL_TOKEN_RESERVES);
    if (opts.tokenAmountRaw * 4n >= realReserves) {
        throw new Error('buy amount must stay below one quarter of remaining real curve tokens');
    }
    const buybackRecipient = getAddressDecoder().decode(
        global.subarray(OFF_GLOBAL_BUYBACK_FEE_RECIPIENTS, OFF_GLOBAL_BUYBACK_FEE_RECIPIENTS + 32),
    );
    if (buybackRecipient === SYSTEM) throw new Error('Pump Global buyback recipient is unset');
    const buybackAccount = (
        await rpc.getAccountInfo(buybackRecipient, { encoding: 'base64', commitment: 'confirmed' }).send()
    ).value;
    if (!buybackAccount || buybackAccount.executable) throw new Error('Pump buyback recipient is absent or executable');
    const existingBuyerAta = (
        await rpc.getAccountInfo(baseBuyerAta, { encoding: 'base64', commitment: 'confirmed' }).send()
    ).value;
    if (existingBuyerAta) {
        const ataData = bytes(existingBuyerAta);
        if (existingBuyerAta.owner !== TOKEN_2022_PROGRAM_ADDRESS || ataData.length < 72) {
            throw new Error('existing buyer token account has an unexpected owner or layout');
        }
        if (ataData.readBigUInt64LE(64) > 0n) {
            throw new Error('buyer already holds this token; this first-inventory packet cannot be repeated');
        }
    }

    const createBuyerAta = await getCreateAssociatedTokenIdempotentInstructionAsync({
        payer: buyer,
        owner: buyer.address,
        mint: opts.mint,
        tokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
    });
    const buy = {
        programAddress: PUMP,
        accounts: [
            meta(GLOBAL, false),
            meta(opts.mint, false),
            meta(WSOL, false),
            meta(TOKEN_2022_PROGRAM_ADDRESS, false),
            meta(TOKEN_PROGRAM, false),
            meta(curve, true),
            meta(baseCurveAta, true),
            meta(quoteCurveAta, true),
            { address: buyer.address, role: AccountRole.WRITABLE_SIGNER, signer: buyer } as AccountMeta,
            meta(baseBuyerAta, true),
            meta(quoteBuyerAta, true),
            meta(userVolumeAccumulator, true),
            meta(FEE_CONFIG, false),
            meta(buybackRecipient, true),
            meta(SYSTEM, false),
            meta(EVENT_AUTHORITY, false),
            meta(PUMP, false),
        ],
        data: buyV3Data(opts.tokenAmountRaw, opts.maxSolCostLamports),
    } as const;
    const { value: lifetime } = await rpc.getLatestBlockhash({ commitment: 'confirmed' }).send();
    const message = appendTransactionMessageInstructions(
        [createBuyerAta, buy],
        setTransactionMessageLifetimeUsingBlockhash(
            lifetime,
            setTransactionMessageFeePayerSigner(buyer, createTransactionMessage({ version: 0 })),
        ),
    );
    const unsigned = compileTransaction(message);
    const size = getTransactionSize(unsigned);
    if (size > 1232) throw new Error(`transaction is ${size} bytes, above legacy/v0 limit`);
    const buyerBefore = (await rpc.getBalance(buyer.address, { commitment: 'confirmed' }).send()).value;
    if (buyerBefore < opts.maxPayerDebitLamports)
        throw new Error('buyer balance is below selected total-debit ceiling');
    const simulation = await rpc
        .simulateTransaction(getBase64EncodedWireTransaction(unsigned), {
            encoding: 'base64',
            sigVerify: false,
            replaceRecentBlockhash: true,
            commitment: 'confirmed',
            accounts: { addresses: [buyer.address, baseBuyerAta, curve], encoding: 'base64' },
        })
        .send();
    const simulatedBuyer = simulation.value.accounts?.[0];
    const debit = simulatedBuyer ? buyerBefore - simulatedBuyer.lamports : null;
    const report = {
        cluster: 'devnet',
        genesis,
        program: PUMP,
        instruction: 'create buyer ATA + buy_v3',
        feePayer: buyer.address,
        mint: opts.mint,
        bondingCurve: curve,
        buyerTokenAccount: baseBuyerAta,
        buybackFeeRecipient: buybackRecipient,
        tokenAmountRaw: opts.tokenAmountRaw.toString(),
        maxSolCostLamports: opts.maxSolCostLamports.toString(),
        maxPayerDebitLamports: opts.maxPayerDebitLamports.toString(),
        simulatedPayerDebitLamports: debit?.toString() ?? null,
        remainingRealTokenReserves: realReserves.toString(),
        transactionBytes: size,
        unitsConsumed: simulation.value.unitsConsumed?.toString() ?? null,
        simulationError: simulation.value.err,
    };
    console.log(JSON.stringify(report, null, 2));
    if (simulation.value.err !== null) {
        console.error('simulation logs:', ...(simulation.value.logs ?? []));
        throw new Error('unsigned Pump BuyV3 simulation failed; no transaction signed');
    }
    if (debit === null || debit < 0n || debit > opts.maxPayerDebitLamports) {
        throw new Error('simulated payer debit is missing or exceeds the cap; no transaction signed');
    }
    if (!opts.send) {
        console.log('Dry-run complete. No transaction was signed or sent.');
        return;
    }
    const signed = await signTransactionMessageWithSigners(message);
    const signature = getSignatureFromTransaction(signed);
    const sent = await rpc
        .sendTransaction(getBase64EncodedWireTransaction(signed), {
            encoding: 'base64',
            skipPreflight: false,
            preflightCommitment: 'confirmed',
            maxRetries: 3n,
        })
        .send();
    if (sent !== signature) throw new Error(`RPC returned ${sent} for signed transaction ${signature}`);
    console.log(`sent ${signature}`);
    for (let i = 0; i < 45; i += 1) {
        const status = (await rpc.getSignatureStatuses([signature], { searchTransactionHistory: true }).send())
            .value[0];
        if (status?.err) throw new Error(`transaction failed: ${JSON.stringify(status.err)}`);
        if (status?.confirmationStatus === 'confirmed' || status?.confirmationStatus === 'finalized') {
            console.log(`confirmed ${signature} at slot ${status.slot}`);
            return;
        }
        await new Promise((resolve) => setTimeout(resolve, 1_000));
    }
    throw new Error(`confirmation timed out; inspect ${signature} before rerunning`);
}

main().catch((error: unknown) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
});

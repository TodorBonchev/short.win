/**
 * A bounded, devnet-only Pump `create_v2` packet for ShortWin's first market smoke.
 *
 * The account order and discriminator are pinned to pump-fun/pump-public-docs commit
 * 639b87fb45c3adeffa6f7c4a1bf0cb85e392bf22, idl/pump.json SHA-256
 * 7616681ea557975f9f623ac211080f22a93060781eaf4765b0a488d980997207.
 * https://github.com/pump-fun/pump-public-docs/blob/639b87fb45c3adeffa6f7c4a1bf0cb85e392bf22/docs/instructions/COIN_CREATION.md
 *
 * Dry-run never signs or sends. `--send` requires a successful unsigned simulation,
 * a matching public metadata document and a finite simulated payer debit.
 * Keep both keypair files outside Git in mode 0600. This creates a curve only;
 * a separate bounded Pump buy is required to acquire lender inventory.
 *
 * Example:
 *   node scripts/devnet-create-pump-coin.ts \
 *     --payer-keypair /private/payer.json --mint-keypair /private/mint.json
 * Add --send only after the printed simulation and packet have been reviewed.
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
    getAddressEncoder,
    getBase64EncodedWireTransaction,
    getProgramDerivedAddress,
    getSignatureFromTransaction,
    getTransactionSize,
    getUtf8Encoder,
    type Instruction,
    setTransactionMessageFeePayerSigner,
    setTransactionMessageLifetimeUsingBlockhash,
    signTransactionMessageWithSigners,
} from '@solana/kit';
import {
    ASSOCIATED_TOKEN_PROGRAM_ADDRESS,
    findAssociatedTokenPda,
    TOKEN_2022_PROGRAM_ADDRESS,
} from '@solana-program/token-2022';

const RPC_URL = 'https://api.devnet.solana.com';
const DEVNET_GENESIS = 'EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG';
const PUMP = '6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P' as Address;
const MAYHEM = 'MAyhSmzXzV1pTf7LsNkrNwkWKTo4ougAJ1PPg47MD4e' as Address;
const GLOBAL = '4wTV1YmiEkRvAtNtsSGPtUrqRYQMe5SKy2uB4Jjaxnjf' as Address;
const EVENT_AUTHORITY = 'Ce6TQqeHC9p8KetsN6JsjHK7UTZk7nasjjnr7XxXp9F1' as Address;
const SYSTEM = '11111111111111111111111111111111' as Address;
const UPGRADEABLE_LOADER = 'BPFLoaderUpgradeab1e11111111111111111111111' as Address;
const EXPECTED_PUMP_DEPLOY_SLOT = 510_657_843n;
const CREATE_V2_DISCRIMINATOR = Uint8Array.of(214, 144, 76, 236, 95, 139, 49, 180);
const DEFAULT_URI =
    'https://raw.githubusercontent.com/TodorBonchev/short.win/main/docs/static/shortwin-devnet-smoke-token-2026-10-23.json';
const DEFAULT_MAX_SPEND_LAMPORTS = 300_000_000n;
const METADATA_TIMEOUT_MS = 10_000;

type Options = {
    payerKeypair: string;
    mintKeypair: string;
    name: string;
    symbol: string;
    uri: string;
    maxSpendLamports: bigint;
    send: boolean;
};

function parseArgs(argv: readonly string[]): Options {
    const values = new Map<string, string>();
    let send = false;
    for (let i = 0; i < argv.length; i += 1) {
        const flag = argv[i];
        if (flag === '--send') {
            if (send) throw new Error('duplicate --send');
            send = true;
            continue;
        }
        if (
            !flag?.startsWith('--') ||
            !['--payer-keypair', '--mint-keypair', '--name', '--symbol', '--uri', '--max-spend-lamports'].includes(flag)
        ) {
            throw new Error(`unknown option ${flag ?? '(empty)'}`);
        }
        if (values.has(flag)) throw new Error(`duplicate ${flag}`);
        const value = argv[++i];
        if (!value || value.startsWith('--')) throw new Error(`${flag} needs a value`);
        values.set(flag, value);
    }
    const payerKeypair = values.get('--payer-keypair');
    const mintKeypair = values.get('--mint-keypair');
    if (!payerKeypair || !mintKeypair) {
        throw new Error('both --payer-keypair and --mint-keypair are required');
    }
    const name = values.get('--name') ?? 'ShortWin Devnet Test';
    const symbol = values.get('--symbol') ?? 'SWTEST';
    const uri = values.get('--uri') ?? DEFAULT_URI;
    if (!/^[\x20-\x7e]{1,32}$/.test(name)) throw new Error('name must be 1–32 printable ASCII characters');
    if (!/^[A-Z0-9]{1,13}$/.test(symbol)) throw new Error('symbol must be 1–13 uppercase letters or digits');
    if (uri.length > 200 || !uri.startsWith('https://'))
        throw new Error('uri must be HTTPS and at most 200 characters');
    const spendText = values.get('--max-spend-lamports') ?? DEFAULT_MAX_SPEND_LAMPORTS.toString();
    if (!/^[1-9][0-9]*$/.test(spendText)) throw new Error('max spend must be a positive integer in lamports');
    const maxSpendLamports = BigInt(spendText);
    if (maxSpendLamports > 1_000_000_000n) throw new Error('this smoke caps coin creation at 1 devnet SOL');
    return { payerKeypair, mintKeypair, name, symbol, uri, maxSpendLamports, send };
}

async function signerFromFile(path: string) {
    if ((statSync(path).mode & 0o077) !== 0) throw new Error('keypair file must not be group/world accessible');
    const raw: unknown = JSON.parse(readFileSync(path, 'utf8'));
    if (!Array.isArray(raw) || raw.length !== 64 || !raw.every((n) => Number.isInteger(n) && n >= 0 && n <= 255)) {
        throw new Error('expected a Solana 64-byte JSON keypair file');
    }
    return createKeyPairSignerFromBytes(Uint8Array.from(raw));
}

function borshString(value: string): Uint8Array {
    const data = new TextEncoder().encode(value);
    const result = new Uint8Array(4 + data.length);
    new DataView(result.buffer).setUint32(0, data.length, true);
    result.set(data, 4);
    return result;
}

export function createV2Data(name: string, symbol: string, uri: string, creator: Address): Uint8Array {
    const fields = [
        CREATE_V2_DISCRIMINATOR,
        borshString(name),
        borshString(symbol),
        borshString(uri),
        getAddressEncoder().encode(creator),
        Uint8Array.of(0),
    ];
    const out = new Uint8Array(fields.reduce((sum, field) => sum + field.length, 0));
    let offset = 0;
    for (const field of fields) {
        out.set(field, offset);
        offset += field.length;
    }
    // The three trailing OptionBool/OptionU64/OptionBool args are omitted. Pump's published
    // CreateV2 guide says omission decodes as false / 0 / false: no cashback or holder rewards.
    return out;
}

function meta(address: Address, writable: boolean): AccountMeta {
    return { address, role: writable ? AccountRole.WRITABLE : AccountRole.READONLY };
}

async function deriveAddresses(mint: Address) {
    const utf8 = getUtf8Encoder();
    const encodedMint = getAddressEncoder().encode(mint);
    const [mintAuthority] = await getProgramDerivedAddress({
        programAddress: PUMP,
        seeds: [utf8.encode('mint-authority')],
    });
    const [bondingCurve] = await getProgramDerivedAddress({
        programAddress: PUMP,
        seeds: [utf8.encode('bonding-curve'), encodedMint],
    });
    const [associatedBondingCurve] = await findAssociatedTokenPda({
        owner: bondingCurve,
        mint,
        tokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
    });
    const [globalParams] = await getProgramDerivedAddress({
        programAddress: MAYHEM,
        seeds: [utf8.encode('global-params')],
    });
    const [solVault] = await getProgramDerivedAddress({ programAddress: MAYHEM, seeds: [utf8.encode('sol-vault')] });
    const [mayhemState] = await getProgramDerivedAddress({
        programAddress: MAYHEM,
        seeds: [utf8.encode('mayhem-state'), encodedMint],
    });
    const [mayhemTokenVault] = await findAssociatedTokenPda({
        owner: solVault,
        mint,
        tokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
    });
    const [programData] = await getProgramDerivedAddress({
        programAddress: UPGRADEABLE_LOADER,
        seeds: [getAddressEncoder().encode(PUMP)],
    });
    return {
        mintAuthority,
        bondingCurve,
        associatedBondingCurve,
        globalParams,
        solVault,
        mayhemState,
        mayhemTokenVault,
        programData,
    };
}

async function checkMetadata(opts: Options): Promise<void> {
    const response = await fetch(opts.uri, { signal: AbortSignal.timeout(METADATA_TIMEOUT_MS) });
    if (!response.ok) throw new Error(`metadata URI returned HTTP ${response.status}`);
    const data: unknown = await response.json();
    if (!data || typeof data !== 'object') throw new Error('metadata URI did not return a JSON object');
    const obj = data as Record<string, unknown>;
    if (
        obj.name !== opts.name ||
        obj.symbol !== opts.symbol ||
        typeof obj.description !== 'string' ||
        !/devnet/i.test(obj.description)
    ) {
        throw new Error('metadata URI must match name/symbol and describe this as devnet-only');
    }
}

async function main(): Promise<void> {
    const opts = parseArgs(process.argv.slice(2));
    const payer = await signerFromFile(opts.payerKeypair);
    const mint = await signerFromFile(opts.mintKeypair);
    if (payer.address === mint.address) throw new Error('payer and new mint must be separate signers');
    const rpc = createSolanaRpc(RPC_URL);
    const genesis = await rpc.getGenesisHash().send();
    if (genesis !== DEVNET_GENESIS) throw new Error(`wrong cluster genesis ${genesis}`);
    const a = await deriveAddresses(mint.address);
    const checked = await rpc
        .getMultipleAccounts(
            [PUMP, MAYHEM, GLOBAL, a.globalParams, a.solVault, a.programData, mint.address, a.bondingCurve],
            { encoding: 'base64', commitment: 'confirmed' },
        )
        .send();
    const [pumpProgram, mayhemProgram, global, globalParams, solVault, programData, mintAccount, curveAccount] =
        checked.value;
    if (!pumpProgram?.executable || !mayhemProgram?.executable)
        throw new Error('Pump or Mayhem is not executable on devnet');
    if (global?.owner !== PUMP || globalParams?.owner !== MAYHEM || solVault?.owner !== SYSTEM)
        throw new Error('Pump/Mayhem canonical config account owner mismatch');
    if (mintAccount || curveAccount) throw new Error('mint or bonding curve already exists');
    if (programData?.owner !== UPGRADEABLE_LOADER) throw new Error('Pump ProgramData owner mismatch');
    const pd = Buffer.from(programData.data[0], 'base64');
    if (pd.length < 45 || pd.readUInt32LE(0) !== 3 || pd.readBigUInt64LE(4) !== EXPECTED_PUMP_DEPLOY_SLOT) {
        throw new Error('Pump devnet deployment changed; revalidate the CreateV2 interface before proceeding');
    }
    if (opts.send) await checkMetadata(opts);

    const accounts: AccountMeta[] = [
        { address: mint.address, role: AccountRole.WRITABLE_SIGNER, signer: mint } as AccountMeta,
        meta(a.mintAuthority, false),
        meta(a.bondingCurve, true),
        meta(a.associatedBondingCurve, true),
        meta(GLOBAL, false),
        { address: payer.address, role: AccountRole.WRITABLE_SIGNER, signer: payer } as AccountMeta,
        meta(SYSTEM, false),
        meta(TOKEN_2022_PROGRAM_ADDRESS, false),
        meta(ASSOCIATED_TOKEN_PROGRAM_ADDRESS, false),
        meta(MAYHEM, true),
        meta(a.globalParams, false),
        meta(a.solVault, true),
        meta(a.mayhemState, true),
        meta(a.mayhemTokenVault, true),
        meta(EVENT_AUTHORITY, false),
        meta(PUMP, false),
    ];
    const instruction: Instruction = {
        programAddress: PUMP,
        accounts,
        data: createV2Data(opts.name, opts.symbol, opts.uri, payer.address),
    };
    const { value: lifetime } = await rpc.getLatestBlockhash({ commitment: 'confirmed' }).send();
    const message = appendTransactionMessageInstructions(
        [instruction],
        setTransactionMessageLifetimeUsingBlockhash(
            lifetime,
            setTransactionMessageFeePayerSigner(payer, createTransactionMessage({ version: 0 })),
        ),
    );
    const unsigned = compileTransaction(message);
    const size = getTransactionSize(unsigned);
    if (size > 1232) throw new Error(`transaction is ${size} bytes, above legacy/v0 limit`);
    const payerBefore = (await rpc.getBalance(payer.address, { commitment: 'confirmed' }).send()).value;
    if (payerBefore < opts.maxSpendLamports) throw new Error('fee payer balance is below the selected spend ceiling');
    const unsignedWire = getBase64EncodedWireTransaction(unsigned);
    const simulation = await rpc
        .simulateTransaction(unsignedWire, {
            encoding: 'base64',
            sigVerify: false,
            replaceRecentBlockhash: true,
            commitment: 'confirmed',
            accounts: { addresses: [payer.address, mint.address, a.bondingCurve], encoding: 'base64' },
        })
        .send();
    const simulatedPayer = simulation.value.accounts?.[0];
    const debit = simulatedPayer ? payerBefore - simulatedPayer.lamports : null;
    const report = {
        cluster: 'devnet',
        genesis,
        program: PUMP,
        instruction: 'create_v2',
        feePayer: payer.address,
        mint: mint.address,
        bondingCurve: a.bondingCurve,
        tokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
        name: opts.name,
        symbol: opts.symbol,
        metadataUri: opts.uri,
        mayhem: false,
        cashback: false,
        holderReward: false,
        maxPayerDebitLamports: opts.maxSpendLamports.toString(),
        simulatedPayerDebitLamports: debit?.toString() ?? null,
        transactionBytes: size,
        unitsConsumed: simulation.value.unitsConsumed?.toString() ?? null,
        simulationError: simulation.value.err,
    };
    console.log(JSON.stringify(report, null, 2));
    if (simulation.value.err !== null) {
        console.error('simulation logs:', ...(simulation.value.logs ?? []));
        throw new Error('unsigned Pump CreateV2 simulation failed; no transaction signed');
    }
    if (debit === null || debit < 0n || debit > opts.maxSpendLamports) {
        throw new Error('simulated payer debit is missing or exceeds the spend ceiling; no transaction signed');
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

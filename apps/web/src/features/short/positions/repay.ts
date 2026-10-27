/** Whole-debt token repayment: direct account reads, stored-basis rent and bounded SDK plan. */
import {
    findMarketPda,
    findMarketVaultPda,
    findPoolVaultPda,
    findPositionPda,
    getMarketDecoder,
    getMarketSize,
    getRepayShortInstructionAsync,
    getShortPositionDecoder,
    MARKET_DISCRIMINATOR,
    type Market,
    SHORTWIN_ERROR__INSUFFICIENT_VAULT,
    SHORTWIN_ERROR__VENUE_RESULT_MISMATCH,
    type ShortPosition,
} from '@shortwin/client';
import { type Address, getBase64Encoder, type TransactionSigner } from '@solana/kit';
import { qk } from '../../../api/keys.ts';
import { SHORTWIN_PROGRAM, TOKEN_2022_PROGRAM, TOKEN_PROGRAM } from '../../../config/programs.ts';
import { cleanSymbol, tokenLabel } from '../../../lib/displayText.ts';
import { shortAddress } from '../../../lib/format.ts';
import type { MathModule } from '../../../math/load.ts';
import type { AppRpc } from '../../../rpc/client.ts';
import { type MintInfo, parseMint } from '../../../rpc/mint.ts';
import { eventAuthority } from '../../../rpc/pda.ts';
import { PlanError } from '../../../tx/errors.ts';
import { parseTokenAccount, TOKEN_ERROR_CPI_GUARD_TRANSFER_BLOCKED } from '../../../tx/tokenAccount.ts';
import type { SimulationReport, TxPlan } from '../../../tx/types.ts';
import { CLOCK_SYSVAR, fetchRaw } from '../../../venues/fetch.ts';
import { readTokenAccount, value } from '../../../venues/readers.ts';
import { readShortPosition } from '../positionAccount.ts';
import { rentBasisNow } from '../previews.ts';
import { plainAmount } from '../ui.tsx';

export interface RepaySource {
    readonly address: Address;
    readonly programAddress: Address;
    readonly mint: Address;
    readonly owner: Address;
    readonly amount: bigint;
    readonly cpiGuard: boolean;
    readonly frozen: boolean;
}

export interface RepayState {
    readonly positionAddress: Address;
    readonly position: ShortPosition;
    readonly marketAddress: Address;
    readonly market: Market;
    readonly mintInfo: MintInfo;
    readonly sources: readonly RepaySource[];
    readonly walletLamports: bigint | null;
    readonly now: bigint;
    readonly slot: bigint;
    readonly readAt: number;
}

/** Verify token identity/owner/state from bytes, not only getTokenAccountsByOwner's filter. */
export function repaySource(
    account: { readonly address: Address; readonly owner: Address; readonly data: Uint8Array },
    borrower: Address,
    mint: Address,
    tokenProgram: Address,
): RepaySource | null {
    if (account.owner !== tokenProgram) return null;
    const base = value(readTokenAccount(account.owner, account.data));
    const extensions = parseTokenAccount(account.address, account.owner, account.data);
    if (!base || !extensions || base.mint !== mint || base.owner !== borrower) return null;
    return {
        address: account.address,
        programAddress: account.owner,
        mint: base.mint,
        owner: base.owner,
        amount: base.amount,
        cpiGuard: extensions.cpiGuard,
        frozen: base.frozen,
    };
}

/** No venue, oracle or quote read. A missing/malformed Clock cannot become a device-time rent quote. */
export async function readRepayState(
    rpc: AppRpc,
    positionAddress: Address,
    owner: Address | null,
): Promise<RepayState> {
    const position = await readShortPosition(rpc, positionAddress);
    if (!position) throw new PlanError('This position is no longer open. Refresh its history.');
    const first = await fetchRaw(rpc, [position.market]);
    const raw = first.accounts.get(position.market);
    if (
        !raw ||
        raw.owner !== SHORTWIN_PROGRAM ||
        raw.executable ||
        raw.data.length < getMarketSize() ||
        !MARKET_DISCRIMINATOR.every((byte, i) => raw.data[i] === byte)
    ) {
        throw new PlanError('The position’s market account could not be verified.');
    }
    const market = getMarketDecoder().decode(raw.data);
    const [expectedMarket] = await findMarketPda({ mint: market.mint });
    if (
        expectedMarket !== position.market ||
        (market.tokenProgram !== TOKEN_PROGRAM && market.tokenProgram !== TOKEN_2022_PROGRAM)
    ) {
        throw new PlanError('The position’s market or token program does not match.');
    }
    const second = await fetchRaw(rpc, [market.mint, CLOCK_SYSVAR]);
    const mint = second.accounts.get(market.mint);
    const mintInfo = mint ? parseMint(market.mint, mint.owner, mint.data) : null;
    if (
        !mint ||
        mint.owner !== market.tokenProgram ||
        !mintInfo?.initialized ||
        mintInfo.decimals !== market.decimals
    ) {
        throw new PlanError('The mint’s token program or decimals no longer match this market.');
    }
    const clock = second.accounts.get(CLOCK_SYSVAR);
    if (clock?.owner !== 'Sysvar1111111111111111111111111111111111111' || clock.data.length < 40) {
        throw new PlanError('The chain clock is unavailable. Refresh before estimating rent.');
    }
    const now = new DataView(clock.data.buffer, clock.data.byteOffset, clock.data.byteLength).getBigInt64(32, true);
    const sources: RepaySource[] = [];
    let walletLamports: bigint | null = null;
    if (owner === position.borrower) {
        const [tokens, balance] = await Promise.all([
            rpc
                .getTokenAccountsByOwner(owner, { mint: market.mint }, { encoding: 'base64', commitment: 'confirmed' })
                .send(),
            rpc.getBalance(owner, { commitment: 'confirmed' }).send(),
        ]);
        const b64 = getBase64Encoder();
        for (const { pubkey, account } of tokens.value) {
            const source = repaySource(
                { address: pubkey, owner: account.owner, data: new Uint8Array(b64.encode(account.data[0])) },
                owner,
                market.mint,
                market.tokenProgram,
            );
            if (source) sources.push(source);
        }
        walletLamports = balance.value;
    }
    sources.sort((a, b) => (a.amount === b.amount ? a.address.localeCompare(b.address) : a.amount > b.amount ? -1 : 1));
    return {
        positionAddress,
        position,
        marketAddress: position.market,
        market,
        mintInfo,
        sources,
        walletLamports,
        now,
        slot: second.slot,
        readAt: Date.now(),
    };
}

export interface RepayPreview {
    readonly rentDue: bigint;
    readonly vaultCanPay: boolean;
    readonly estimatedVaultReturn: bigint | null;
}

/** Same rent basis and locked PositionRent as the program, without a buyback or price-source read. */
export function previewRepay(
    math: MathModule,
    state: Pick<RepayState, 'market' | 'position'>,
    now: bigint,
): RepayPreview {
    const basis = rentBasisNow(math, { market: state.market, now });
    const pos = state.position;
    const opened = new math.RentBasis(pos.jOpen[0] ?? 0n, pos.jOpen[1] ?? 0n, pos.jOpen[2] ?? 0n);
    const rent = new math.PositionRent(
        pos.q,
        opened,
        pos.rateBaseBps,
        pos.rateSlope1Bps,
        pos.rateSlope2Bps,
        pos.rentProtocolBps,
        pos.rMin,
    );
    try {
        const rentDue = rent.due(basis);
        return {
            rentDue,
            vaultCanPay: rentDue <= pos.v,
            estimatedVaultReturn: rentDue <= pos.v ? pos.v - rentDue : null,
        };
    } finally {
        rent.free();
        opened.free();
        basis.free();
    }
}

/** One source must hold the full debt. No top-level token transfer/authority permission is added. */
export function repaymentReason(
    state: RepayState,
    signer: Address,
    source: RepaySource | null,
    maxRent: bigint | null,
    preview: RepayPreview | null,
): string | null {
    if (signer !== state.position.borrower) return 'This position belongs to another wallet';
    if (state.position.q <= 0n) return 'This position has no open token debt';
    if (
        !state.mintInfo.initialized ||
        state.mintInfo.address !== state.market.mint ||
        state.mintInfo.decimals !== state.market.decimals ||
        (state.mintInfo.program === 'token' ? TOKEN_PROGRAM : TOKEN_2022_PROGRAM) !== state.market.tokenProgram
    ) {
        return 'The mint identity, decimals or token program do not match this market';
    }
    if (!source) return 'Choose a token account that holds the full debt';
    if (
        source.owner !== signer ||
        source.mint !== state.market.mint ||
        source.programAddress !== state.market.tokenProgram
    ) {
        return 'The token account does not match your wallet and this mint';
    }
    if (source.frozen) return 'This token account is frozen';
    if (source.cpiGuard)
        return 'CPI Guard is enabled on this token account. Use another account or turn it off in your wallet.';
    if (source.amount < state.position.q) return 'One token account must hold the full token debt';
    if (maxRent === null || maxRent < 0n || maxRent > (1n << 64n) - 1n) return 'Enter an explicit maximum rent in SOL';
    if (maxRent > state.position.v) return 'The rent limit cannot exceed this position’s vault';
    if (!preview) return 'Accrued rent could not be estimated from the current state';
    if (!preview.vaultCanPay) return 'Add margin first: the vault cannot cover the accrued rent';
    if (maxRent < preview.rentDue) return 'The rent limit is below the current accrued rent';
    if (state.walletLamports === null) return 'Reading your wallet’s SOL balance';
    if (state.walletLamports <= 0n) return 'Your wallet needs SOL to pay the network fee';
    return null;
}

export interface RepayInputs {
    readonly signer: TransactionSigner;
    readonly source: Address;
    readonly maxRent: bigint;
    readonly symbol: string;
}

export function repaymentSimulationHold(
    report: SimulationReport,
    source: Address,
    pool: Address,
    q: bigint,
    payer?: { readonly address: Address; readonly lamports: bigint | null },
): string | null {
    if (payer) {
        const beforeRefunds =
            report.deltas.find((delta) => delta.key === `${payer.address}:lamports`)?.before ?? payer.lamports;
        if (beforeRefunds === null || beforeRefunds < report.feeLamports) {
            return 'The simulated network fee exceeds your wallet’s SOL balance before account refunds.';
        }
    }
    const input = report.deltas.find((delta) => delta.key === `${source}:amount`);
    const output = report.deltas.find((delta) => delta.key === `${pool}:amount`);
    if (
        input?.before === null ||
        input?.before === undefined ||
        input.after === null ||
        output?.before === null ||
        output?.before === undefined ||
        output.after === null ||
        input.before - input.after !== q ||
        output.after - output.before !== q
    ) {
        return 'The simulation did not verify the full token repayment. Refresh and review again.';
    }
    return null;
}

export async function planRepay(
    math: MathModule,
    state: RepayState,
    inputs: RepayInputs,
    now = state.now,
): Promise<TxPlan> {
    const { signer, maxRent, symbol } = inputs;
    const pos = state.position;
    const unit = cleanSymbol(symbol) ?? shortAddress(state.market.mint);
    const source = state.sources.find((account) => account.address === inputs.source) ?? null;
    const reason = repaymentReason(state, signer.address, source, maxRent, previewRepay(math, state, now));
    if (reason || !source) throw new PlanError(reason ?? 'Select a token account');
    const [[market], [position], [pool], [vault]] = await Promise.all([
        findMarketPda({ mint: state.market.mint }),
        findPositionPda({ market: state.marketAddress, borrower: pos.borrower, nonce: pos.nonce }),
        findPoolVaultPda({ market: state.marketAddress }),
        findMarketVaultPda({ market: state.marketAddress }),
    ]);
    if (
        pos.market !== state.marketAddress ||
        market !== state.marketAddress ||
        position !== state.positionAddress ||
        state.market.poolVault !== pool ||
        state.market.marketVault !== vault
    ) {
        throw new PlanError('The position or vault identities do not match this market.');
    }
    const ix = await getRepayShortInstructionAsync({
        borrower: signer,
        market: state.marketAddress,
        position: state.positionAddress,
        mint: state.market.mint,
        borrowerTokenAccount: source.address,
        poolVault: pool,
        marketVault: vault,
        tokenProgram: state.market.tokenProgram,
        eventAuthority: await eventAuthority(),
        program: SHORTWIN_PROGRAM,
        maxRent,
    });
    const decoder = getShortPositionDecoder();
    return {
        kind: 'repay_short',
        title: `Repay tokens · ${plainAmount(pos.q, state.mintInfo.decimals)} ${tokenLabel(symbol, state.market.mint)} · max rent ${plainAmount(maxRent, 9)} SOL`,
        instructions: [ix],
        watch: [
            { kind: 'lamports', address: signer.address, label: 'Your SOL (including account refunds)' },
            {
                kind: 'token',
                address: source.address,
                label: `Your tokens (${tokenLabel(symbol, state.market.mint)})`,
                decimals: state.mintInfo.decimals,
                symbol: unit,
            },
            { kind: 'token', address: pool, label: 'Pool tokens repaid', decimals: state.mintInfo.decimals, symbol },
            {
                kind: 'program',
                address: state.positionAddress,
                label: 'Position',
                decode: (data) => {
                    const decoded = decoder.decode(data);
                    return { q: decoded.q, v: decoded.v };
                },
                fields: {
                    q: { label: 'Debt', unit, scale: state.mintInfo.decimals },
                    v: { label: 'Vault', unit: 'SOL', scale: 9 },
                },
            },
        ],
        disclosures: ['borrower.tokenRepay'],
        disclosureVars: {
            'borrower.tokenRepay': {
                q: plainAmount(pos.q, state.mintInfo.decimals),
                qRaw: pos.q.toString(),
                maxRent: plainAmount(maxRent, 9),
                maxRentLamports: maxRent.toString(),
            },
        },
        refineError: (error) => {
            if (error.kind === 'program' && error.code === SHORTWIN_ERROR__INSUFFICIENT_VAULT) {
                return {
                    ...error,
                    message: 'The position vault cannot cover the full accrued rent. Add margin before repaying.',
                };
            }
            if (error.kind === 'program' && error.code === SHORTWIN_ERROR__VENUE_RESULT_MISMATCH) {
                return {
                    ...error,
                    message:
                        'The token transfer did not deliver the exact full debt to the pool. Transfer fees or unsupported token rules may block repayment.',
                };
            }
            if (error.kind === 'token' && error.code === TOKEN_ERROR_CPI_GUARD_TRANSFER_BLOCKED) {
                return {
                    ...error,
                    message:
                        'CPI Guard blocks this token repayment. Use another account or turn it off in your wallet before reviewing again.',
                };
            }
            return error;
        },
        holdSign: (report) =>
            repaymentSimulationHold(report, source.address, pool, pos.q, {
                address: signer.address,
                lamports: state.walletLamports,
            }),
        invalidate: [
            qk.positionAccount(state.positionAddress),
            qk.position(state.positionAddress),
            qk.marketAccount(state.market.mint),
            qk.market(state.market.mint),
            qk.exposure(state.market.mint, signer.address),
            qk.tokenAccounts(signer.address, state.market.mint),
            qk.balance(signer.address),
            ['indexer', 'borrower', signer.address],
        ],
    };
}

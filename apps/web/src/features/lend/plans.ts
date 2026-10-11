/**
 * `TxPlan` builders of the lender flows (FRONTEND.md §3.6, §5.1, §5.7, §5.10): `deposit` (with the
 * CPI Guard bracket of FE-1 and the post-wipe sweep bundle), `withdraw` (an idempotent ATA for a
 * missing destination; `memo_program` for a memo-required one), `claim_sol` (other lenders'
 * `sync_lender` first when the pot cannot pay: unsigned, so the pot-only touch with its forward
 * chain for a lender behind a rebase record, KI-66 full / KI66-R4), `cover_deficit` and
 * `sync_lender`. Instructions come
 * only from the Codama builders of `@shortwin/client`, `@solana-program/token(-2022)` and the
 * keeper-ported sweep bundle (`sweep.ts`); every signer input is the connected wallet's signer.
 *
 * Every plan carries the `ShareEpoch` records the lender is behind on as remaining accounts
 * (`getShareEpochAccountMetas`), the accounts whose before / after the review shows, the
 * disclosures it needs acknowledged, and the queries to refresh once it lands.
 */
import {
    findShareEpochPda,
    getClaimSolInstructionAsync,
    getCoverDeficitInstructionAsync,
    getDepositInstructionAsync,
    getLenderPositionDecoder,
    getShareEpochAccountMetas,
    getSyncLenderInstructionAsync,
    getWithdrawInstructionAsync,
} from '@shortwin/client';
import { type AccountMeta, AccountRole, type Address, type Instruction, type TransactionSigner } from '@solana/kit';
import {
    getCreateAssociatedTokenIdempotentInstruction,
    getDisableCpiGuardInstruction,
    getEnableCpiGuardInstruction,
} from '@solana-program/token-2022';
import type { QueryKey } from '@tanstack/react-query';
import { qk } from '../../api/keys.ts';
import type { DisclosureId } from '../../config/disclosures.ts';
import { MEMO_PROGRAM, SHORTWIN_PROGRAM, TOKEN_2022_PROGRAM } from '../../config/programs.ts';
import { cleanSymbol, tokenLabel } from '../../lib/displayText.ts';
import { formatSol, formatTokens, shortAddress } from '../../lib/format.ts';
import { eventAuthority } from '../../rpc/pda.ts';
import { PlanError } from '../../tx/errors.ts';
import type { TxPlan, WatchedAccount } from '../../tx/types.ts';
import { lendKeys, MAX_RECORDS } from './chain.ts';

/** What every lender plan needs to know about the market and the wallet. */
export interface LendContext {
    /** The connected wallet's signer: the owner and the fee payer. */
    readonly signer: TransactionSigner;
    readonly mint: Address;
    readonly marketAddress: Address;
    readonly vaultAddress: Address;
    /** The market's token program (`Market.token_program`: SPL Token or Token-2022). */
    readonly tokenProgram: Address;
    readonly decimals: number;
    readonly symbol: string;
    /** The owner's `LenderPosition` PDA. */
    readonly lenderAddress: Address;
    /** The position's share epoch; `null` for a lender that has never deposited. */
    readonly lenderEpoch: number | null;
    readonly shareEpoch: number;
    /**
     * The records the owner's own conversion carries per transaction (`recordLimitOf`):
     * `MAX_RECORDS`, or `MAX_RECORDS_CHAINED` while the position has a forward chain (KI-66 full;
     * the lag-1 CU case, ARCH §6.1). Omitted: `MAX_RECORDS`.
     */
    readonly recordLimit?: number;
    /**
     * `lender.subUnitConversion`'s variables when the plan's own conversion takes the position to 0
     * shares while the pool's income vests (`previewSubUnitConversion`): the review shows the
     * disclosure. Omitted otherwise.
     */
    readonly subUnit?: Readonly<Record<string, string>>;
}

/** The plan's disclosures and their variables, with `lender.subUnitConversion` last when it applies. */
function disclosuresOf(
    c: LendContext,
    ids: readonly DisclosureId[],
    vars?: TxPlan['disclosureVars'],
): Pick<TxPlan, 'disclosures' | 'disclosureVars'> {
    if (!c.subUnit) return { disclosures: ids, disclosureVars: vars };
    return {
        disclosures: [...ids, 'lender.subUnitConversion'],
        disclosureVars: { ...vars, 'lender.subUnitConversion': c.subUnit },
    };
}

/**
 * The `ShareEpoch` records a lender at `fromEpoch` is behind on, as writable remaining accounts: the
 * lender's conversion through a rebase record writes its `conv_rem` (KI-65; `EpochRecordReadOnly`).
 * At most `limit` (`MAX_RECORDS`; `MAX_RECORDS_CHAINED` for an owner settling a forward chain).
 */
export async function recordMetas(
    market: Address,
    fromEpoch: number | null,
    shareEpoch: number,
    limit = MAX_RECORDS,
): Promise<AccountMeta[]> {
    if (fromEpoch === null || fromEpoch >= shareEpoch) return [];
    return getShareEpochAccountMetas({ market, fromEpoch, shareEpoch, limit: Math.min(limit, MAX_RECORDS) });
}

/** The owner's own records for plan `c` (its `recordLimit`). */
const ownRecords = (c: LendContext) =>
    recordMetas(c.marketAddress, c.lenderEpoch, c.shareEpoch, c.recordLimit ?? MAX_RECORDS);

/**
 * The records of `epochs` as **read-only** remaining accounts: a third party's `sync_lender` is the
 * pot-only touch, which writes no record (KI-66 full; the program requires them back unchanged).
 */
export async function readonlyRecordMetas(market: Address, epochs: readonly number[]): Promise<AccountMeta[]> {
    const metas: AccountMeta[] = [];
    for (const epoch of epochs) {
        const [address] = await findShareEpochPda({ market, epoch });
        metas.push({ address, role: AccountRole.READONLY });
    }
    return metas;
}

function withRemaining(ix: Instruction, extra: readonly AccountMeta[]): Instruction {
    return extra.length ? { ...ix, accounts: [...(ix.accounts ?? []), ...extra] } : ix;
}

function lenderWatch(c: LendContext, fields: readonly ('shares' | 'lock' | 'claimed' | 'pending')[]): WatchedAccount {
    const meta = {
        shares: { label: 'shares', unit: 'shares', scale: c.decimals },
        lock: { label: 'locked shares', unit: 'shares', scale: c.decimals },
        claimed: { label: 'SOL claimed', unit: 'SOL', scale: 9 },
        pending: { label: 'SOL owed', unit: 'SOL', scale: 9 },
    } as const;
    const dec = getLenderPositionDecoder();
    return {
        kind: 'program',
        address: c.lenderAddress,
        label: 'Your lender position',
        decode: (data) => {
            const d = dec.decode(data);
            return { shares: d.shares, lock: d.lock, claimed: d.claimed, pending: d.pending };
        },
        fields: Object.fromEntries(fields.map((f) => [f, meta[f]])),
    };
}

/** The unit of a token amount: its (clean) symbol, else the short mint. */
const unitOf = (c: Pick<LendContext, 'symbol' | 'mint'>): string => cleanSymbol(c.symbol) ?? shortAddress(c.mint);

const walletWatch = (c: LendContext): WatchedAccount => ({
    kind: 'lamports',
    address: c.signer.address,
    label: 'Wallet SOL',
});

const vaultWatch = (c: LendContext): WatchedAccount => ({
    kind: 'lamports',
    address: c.vaultAddress,
    label: 'Market SOL vault',
});

/** The queries a landed lender transaction changes. */
export function lendInvalidations(mint: Address, owner: Address): QueryKey[] {
    return [
        qk.marketAccount(mint),
        lendKeys.state(mint, owner),
        qk.tokenAccounts(owner, mint),
        qk.balance(owner),
        qk.lender(owner),
        qk.market(mint),
        qk.marketLenders(mint),
        lendKeys.onChainPositions(owner),
        // The Claim button's search for syncs (`claim/syncSearch.ts`): a landed claim, deposit or
        // withdrawal of this market searches again.
        lendKeys.claimSyncs(mint, owner),
    ];
}

const program = SHORTWIN_PROGRAM;

// ---- deposit ----------------------------------------------------------------------------

export interface DepositPlanInput extends LendContext {
    /** The source token account (any of the owner's accounts of the mint). */
    readonly source: Address;
    readonly sourceCpiGuard: boolean;
    readonly amount: bigint;
    readonly minShares: bigint;
    /** FE-1: turn CPI Guard off for this one deposit (`[DisableCpiGuard, deposit, EnableCpiGuard]`). */
    readonly bracket: boolean;
    /** The post-wipe sweep bundle (`sweep.ts`), when a deposit would otherwise wait for it. */
    readonly sweep: readonly Instruction[];
    /**
     * `lender.unpaidRent`'s variables when the deposit's part of the pool's unpaid rent is material
     * (`previewUnpaidRent` level `warn`): the review repeats the disclosure. Omitted otherwise.
     */
    readonly unpaidRent?: Readonly<Record<string, string>>;
}

export { PlanError };

export async function buildDepositPlan(i: DepositPlanInput): Promise<TxPlan> {
    if (i.amount <= 0n) throw new PlanError('Enter an amount above zero.');
    if (i.sourceCpiGuard && !i.bracket) {
        // Token-2022 refuses short.win's CPI transfer from a guarded account (TokenError 42).
        throw new PlanError('CPI Guard is on for this token account: use another account or opt into the bracket.');
    }
    const deposit = withRemaining(
        await getDepositInstructionAsync({
            owner: i.signer,
            market: i.marketAddress,
            marketVault: i.vaultAddress,
            lender: i.lenderAddress,
            mint: i.mint,
            ownerTokenAccount: i.source,
            tokenProgram: i.tokenProgram,
            eventAuthority: await eventAuthority(),
            program,
            amount: i.amount,
            minShares: i.minShares,
        }),
        await ownRecords(i),
    );
    const bracket = i.bracket && i.sourceCpiGuard;
    const instructions: Instruction[] = [
        ...i.sweep,
        ...(bracket ? [getDisableCpiGuardInstruction({ token: i.source, owner: i.signer })] : []),
        deposit,
        ...(bracket ? [getEnableCpiGuardInstruction({ token: i.source, owner: i.signer })] : []),
    ];
    // The token by symbol AND mint (M9-SEC-5): a symbol alone can impersonate a ticker.
    const amount = `${formatTokens(i.amount, i.decimals)} ${tokenLabel(i.symbol, i.mint)}`;
    const steps = [
        ...(i.sweep.length ? ['post-wipe sweep'] : []),
        ...(bracket ? ['CPI Guard off', 'deposit', 'CPI Guard on'] : []),
    ];
    const disclosures: DisclosureId[] = [
        'lender.solRepayment',
        ...(i.unpaidRent ? (['lender.unpaidRent'] as const) : []),
        ...(bracket ? (['token.cpiGuard'] as const) : []),
    ];
    return {
        kind: 'deposit',
        title: `Deposit ${amount}${steps.length ? ` · ${steps.join(' → ')}` : ''}`,
        instructions,
        watch: [
            walletWatch(i),
            {
                kind: 'token',
                address: i.source,
                label: `Source account (${tokenLabel(i.symbol, i.mint)})`,
                decimals: i.decimals,
                symbol: unitOf(i),
            },
            lenderWatch(i, ['shares', 'lock']),
        ],
        ...disclosuresOf(i, disclosures, i.unpaidRent ? { 'lender.unpaidRent': i.unpaidRent } : undefined),
        invalidate: lendInvalidations(i.mint, i.signer.address),
    };
}

// ---- withdraw ---------------------------------------------------------------------------

export interface WithdrawPlanInput extends LendContext {
    /** The destination token account (any account of this mint the owner chooses; default its ATA). */
    readonly destination: Address;
    /** The destination does not exist yet: it must be the owner's ATA, created idempotently. */
    readonly createDestination: boolean;
    /**
     * `MemoTransfer.require_incoming_transfer_memos` on the destination (Token-2022), as last read: the
     * disclosure and the title. `memo_program` does not depend on it (TMEMO-9).
     */
    readonly memoRequired: boolean;
    readonly shares: bigint;
    readonly minTokens: bigint;
    /** The quoted SOL part (vest included); a lower payout reverts `SolBelowMin` (M1-SEC-2). */
    readonly minSol: bigint;
    /** The lender's share of the unreleased vest the withdrawal gives up (`lender.exitForfeitsVest`); 0 for none. */
    readonly vestingForfeit: bigint;
    /** The lender's whole unreleased vest share (the disclosure's `{vesting}`). */
    readonly vesting: bigint;
}

export async function buildWithdrawPlan(i: WithdrawPlanInput): Promise<TxPlan> {
    if (i.shares <= 0n) throw new PlanError('Enter a number of shares above zero.');
    const pre: Instruction[] = i.createDestination
        ? [
              getCreateAssociatedTokenIdempotentInstruction({
                  payer: i.signer,
                  ata: i.destination,
                  owner: i.signer.address,
                  mint: i.mint,
                  tokenProgram: i.tokenProgram,
              }),
          ]
        : [];
    const withdraw = withRemaining(
        await getWithdrawInstructionAsync({
            owner: i.signer,
            market: i.marketAddress,
            marketVault: i.vaultAddress,
            lender: i.lenderAddress,
            mint: i.mint,
            ownerTokenAccount: i.destination,
            tokenProgram: i.tokenProgram,
            // spl-memo v3 for every Token-2022 destination (TMEMO-9): the program CPIs it only when the
            // destination requires memos at execution, so one that turns memo-required after the read
            // still lands. SPL Token: the program id, Anchor's "none".
            memoProgram: i.tokenProgram === TOKEN_2022_PROGRAM ? MEMO_PROGRAM : SHORTWIN_PROGRAM,
            eventAuthority: await eventAuthority(),
            program,
            shares: i.shares,
            minTokens: i.minTokens,
            minSol: i.minSol,
        }),
        await ownRecords(i),
    );
    const disclosures: DisclosureId[] = [
        ...(i.vestingForfeit > 0n ? (['lender.exitForfeitsVest'] as const) : []),
        ...(i.memoRequired ? (['token.memoRequired'] as const) : []),
    ];
    return {
        kind: 'withdraw',
        title: `Withdraw ${formatTokens(i.shares, i.decimals)} shares · ${tokenLabel(i.symbol, i.mint)}${i.memoRequired ? ' · with memo' : ''}`,
        instructions: [...pre, withdraw],
        watch: [
            walletWatch(i),
            {
                kind: 'token',
                address: i.destination,
                label: `Destination account (${tokenLabel(i.symbol, i.mint)})`,
                decimals: i.decimals,
                symbol: unitOf(i),
            },
            lenderWatch(i, ['shares', 'lock', 'claimed']),
            vaultWatch(i),
        ],
        ...disclosuresOf(
            i,
            disclosures,
            i.vestingForfeit > 0n ? { 'lender.exitForfeitsVest': { vesting: formatSol(i.vesting, 6) } } : undefined,
        ),
        invalidate: lendInvalidations(i.mint, i.signer.address),
    };
}

/** The SOL a simulation of the withdrawal (`min_sol = 0`) shows leaving the market's SOL vault. */
export function solPaidFrom(
    deltas: readonly { key: string; before: bigint | null; after: bigint | null }[],
    vault: Address,
): bigint | null {
    const d = deltas.find((x) => x.key === `${vault}:lamports`);
    if (!d || d.before === null || d.after === null) return null;
    const paid = d.before - d.after;
    return paid > 0n ? paid : 0n;
}

/**
 * The SOL a simulated claim pays: the rise of the position's `claimed` (cumulative SOL paid to it,
 * which only the owner's own claim or withdrawal moves), so a read of the position before and the
 * simulation after that land on different slots cannot make a claim that pays 0 look paying (rent
 * paid into the vault in between moves the vault's lamports, not `claimed`). A claim that closes
 * the position (KI-67) leaves no `claimed` after it: the vault's delta then (`solPaidFrom`).
 */
export function claimPaidFrom(
    deltas: readonly { key: string; before: bigint | null; after: bigint | null }[],
    lender: Address,
    vault: Address,
): bigint | null {
    const d = deltas.find((x) => x.key === `${lender}:claimed`);
    if (!d || d.before === null || d.after === null) return solPaidFrom(deltas, vault);
    const paid = d.after - d.before;
    return paid > 0n ? paid : 0n;
}

// ---- claim, cover, sync -----------------------------------------------------------------

export interface SyncTarget {
    /** Another lender's `LenderPosition` whose sync moves a surplus to the pot. */
    readonly lender: Address;
    /**
     * Its owner (`LenderPosition.owner`; the instruction's `owner` account, **not a signer**: the
     * claimant sends it, so the program runs the pot-only touch `pool::touch_pot`, which never
     * converts through a rebase record, KI65-REV-SEC1 / KI-66 full).
     */
    readonly owner: Address;
    /**
     * The epochs of the records it carries, read-only (`potEpochs`: from its `fwdEpoch` once it has
     * a forward chain, else from its `epoch`; ≤ `MAX_RECORDS`): `sync_lender(epochs.length)`.
     */
    readonly epochs: readonly number[];
    /** What it moves to the pot (`previewTouchPot`), lamports. */
    readonly gain: bigint;
}

export interface ClaimPlanInput extends LendContext {
    /** Other lenders' unsigned `sync_lender` first, ≤ 4, largest gain first (FRONTEND.md §3.6). */
    readonly syncs: readonly SyncTarget[];
    /** The previewed payout (title only; the review shows the simulated one). */
    readonly expected: bigint;
    /**
     * The claim leaves the position closable (`previewClaimOutcome`, KI-67): the program closes it and
     * returns its rent; the review says so (`lender.claimCloses`, `{rent}` in SOL).
     */
    readonly closes?: { readonly rent: bigint };
    /**
     * Syncs the claim needs that this transaction does not carry (the 4-sync cap or the 1,232-byte
     * limit): the review says how many and what they would move (`lender.potSyncsDeferred`).
     */
    readonly deferred?: { readonly count: number; readonly gain: bigint };
}

export const MAX_SYNCS_PER_CLAIM = 4;

/** A third party's `sync_lender(epochs.length)` of `s`, its owner unsigned, its records read-only. */
export async function potSyncInstruction(
    market: Address,
    vault: Address,
    s: SyncTarget,
    ea?: Address,
): Promise<Instruction> {
    const records = s.epochs.slice(0, MAX_RECORDS);
    return withRemaining(
        await getSyncLenderInstructionAsync({
            market,
            marketVault: vault,
            lender: s.lender,
            owner: s.owner,
            eventAuthority: ea ?? (await eventAuthority()),
            program,
            maxEpochs: records.length,
        }),
        await readonlyRecordMetas(market, records),
    );
}

export async function buildClaimPlan(i: ClaimPlanInput): Promise<TxPlan> {
    const ea = await eventAuthority();
    const targets = i.syncs.slice(0, MAX_SYNCS_PER_CLAIM);
    const syncs = await Promise.all(targets.map((s) => potSyncInstruction(i.marketAddress, i.vaultAddress, s, ea)));
    const claim = withRemaining(
        await getClaimSolInstructionAsync({
            owner: i.signer,
            market: i.marketAddress,
            lender: i.lenderAddress,
            marketVault: i.vaultAddress,
            eventAuthority: ea,
            program,
        }),
        await ownRecords(i),
    );
    const potGain = targets.reduce((x, s) => x + s.gain, 0n);
    const disclosures: DisclosureId[] = [
        ...(syncs.length ? (['lender.potSyncs'] as const) : []),
        ...(i.deferred && i.deferred.count > 0 ? (['lender.potSyncsDeferred'] as const) : []),
        ...(i.closes ? (['lender.claimCloses'] as const) : []),
    ];
    const vars: NonNullable<TxPlan['disclosureVars']> = {
        ...(syncs.length
            ? {
                  'lender.potSyncs': {
                      count: String(syncs.length),
                      lenders: syncs.length === 1 ? 'lender' : 'lenders',
                      gain: formatSol(potGain, 9),
                  },
              }
            : {}),
        ...(i.deferred && i.deferred.count > 0
            ? {
                  'lender.potSyncsDeferred': {
                      count: String(i.deferred.count),
                      lenders: i.deferred.count === 1 ? 'lender' : 'lenders',
                      gain: formatSol(i.deferred.gain, 9),
                  },
              }
            : {}),
        ...(i.closes ? { 'lender.claimCloses': { rent: formatSol(i.closes.rent, 9) } } : {}),
    };
    return {
        kind: 'claim_sol',
        title: `Claim ${formatSol(i.expected, 9)} SOL · ${tokenLabel(i.symbol, i.mint)}${syncs.length ? ` · ${syncs.length} lender sync${syncs.length > 1 ? 's' : ''} first` : ''}${i.closes ? ' · closes the position' : ''}`,
        instructions: [...syncs, claim],
        // A claim that closes the position (KI-67) leaves no account: its fields read "—" after.
        watch: [walletWatch(i), lenderWatch(i, ['claimed', 'pending']), vaultWatch(i)],
        ...disclosuresOf(i, disclosures, Object.keys(vars).length ? vars : undefined),
        invalidate: lendInvalidations(i.mint, i.signer.address),
        // The owner's decision of 2026-10-11 (FRONTEND.md §3.6): a claim whose simulation pays 0
        // lamports is not offered for signing (`claimPaidFrom`).
        holdSign: (report) =>
            claimPaidFrom(report.deltas, i.lenderAddress, i.vaultAddress) === 0n ? 'Nothing to claim yet' : null,
    };
}

export interface CoverPlanInput extends LendContext {
    /** The deficit to pay in, lamports (exact: `−pending`). */
    readonly lamports: bigint;
}

export async function buildCoverPlan(i: CoverPlanInput): Promise<TxPlan> {
    if (i.lamports <= 0n) throw new PlanError('There is no deficit to cover.');
    const ix = withRemaining(
        await getCoverDeficitInstructionAsync({
            owner: i.signer,
            market: i.marketAddress,
            lender: i.lenderAddress,
            marketVault: i.vaultAddress,
            eventAuthority: await eventAuthority(),
            program,
            lamports: i.lamports,
        }),
        await ownRecords(i),
    );
    return {
        kind: 'cover_deficit',
        title: `Cover deficit · ${formatSol(i.lamports, 9)} SOL · ${tokenLabel(i.symbol, i.mint)}`,
        instructions: [ix],
        watch: [walletWatch(i), lenderWatch(i, ['pending']), vaultWatch(i)],
        ...disclosuresOf(i, []),
        invalidate: lendInvalidations(i.mint, i.signer.address),
    };
}

/**
 * `sync_lender(n)` of the owner's own position through its next `n ≤ MAX_RECORDS` records, signed
 * by the owner: only the owner converts its position through a rebase record (KI65-REV-SEC1);
 * unsigned, the program would stop there with the pot-only touch (KI-66).
 */
export async function buildSyncPlan(i: LendContext): Promise<TxPlan> {
    const metas = await ownRecords(i);
    const ix = withRemaining(
        await getSyncLenderInstructionAsync({
            market: i.marketAddress,
            marketVault: i.vaultAddress,
            lender: i.lenderAddress,
            owner: i.signer,
            eventAuthority: await eventAuthority(),
            program,
            maxEpochs: metas.length,
        }),
        metas,
    );
    return {
        kind: 'sync_lender',
        title: `Sync position · ${tokenLabel(i.symbol, i.mint)} · ${metas.length} share epoch${metas.length === 1 ? '' : 's'}`,
        instructions: [ix],
        watch: [walletWatch(i), lenderWatch(i, ['shares', 'pending'])],
        ...disclosuresOf(i, []),
        invalidate: lendInvalidations(i.mint, i.signer.address),
    };
}

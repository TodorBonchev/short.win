/**
 * Test helpers for the LEND track: account-shaped fixtures (the decoded `Market`, `MarketVault`,
 * `LenderPosition` and `ShareEpoch` the panel reads) built from the pool_model vectors or from a
 * handful of numbers. Test-only (imported by `*.test.ts(x)`).
 */
import { bigIntToPodI128, bigIntToPodU128, type Market } from '@shortwin/client';
import { type Address, address } from '@solana/kit';
import { type LenderFields, type LendState, MARKET_FLAG, type RecordFields, type VaultPoolFields } from './previews.ts';

const WAD = 10n ** 18n;
export const NO_LOCK_EPOCH = 4_294_967_295;
/** The default (all-zero) key: an unset `Pubkey` field. */
const ZERO = '11111111111111111111111111111111';

export interface VectorCase {
    readonly name: string;
    readonly now: string;
    readonly lockGraceS: number;
    readonly market: Record<string, string | number>;
    readonly vault: {
        readonly lenderSol: string;
        readonly unmatched: string;
        readonly protocolFees: string;
        readonly pi: string;
        readonly lockNorm: string;
        readonly bMin: string;
        readonly piEpoch: number;
        readonly calDay: string;
        readonly lockCal: Readonly<Record<string, string>>;
    };
    readonly lenders: Readonly<Record<string, Record<string, string | number>>>;
    readonly records: readonly Record<string, string | number>[];
    readonly expect: readonly VectorExpect[];
}

export type VectorExpect =
    | { readonly op: 'deposit'; readonly lender: string; readonly amount: string; readonly result: string }
    | {
          readonly op: 'withdraw';
          readonly lender: string;
          readonly shares: string;
          readonly result: string | { readonly tokens: string; readonly sol: string };
      }
    | { readonly op: 'claim' | 'allowance' | 'value' | 'lock'; readonly lender: string; readonly result: string }
    | {
          readonly op: 'touch';
          readonly lender: string;
          readonly result:
              | string
              | {
                    readonly pending: string;
                    readonly real: string;
                    readonly claimed: string;
                    readonly shares: string;
                    readonly lock: string;
                    readonly epoch: number;
                };
      }
    | { readonly op: 'lockUntil' | 'release'; readonly result: string }
    | {
          readonly op: 'touchPot';
          readonly lender: string;
          readonly result:
              | string
              | {
                    readonly potGain: string;
                    readonly standing: string;
                    readonly pending: string;
                    readonly real: string;
                    readonly shares: string;
                    readonly epoch: number;
                    readonly snap: string;
                    readonly snapR: string;
                    readonly fwd: number;
                    readonly fwdEpoch: number;
                    readonly fwdShares: string;
                    readonly fwdLag: number;
                };
      }
    | {
          readonly op: 'claimOutcome';
          readonly lender: string;
          readonly result: string | { readonly sol: string; readonly closes: boolean };
      };

const big = (v: string | number | undefined): bigint => BigInt(v ?? 0);

export function marketOf(m: VectorCase['market'], venue = 0): LendState['market'] {
    return {
        tokensFree: big(m.tokensFree),
        borrowedQ: big(m.borrowedQ),
        totalShares: bigIntToPodU128(big(m.totalShares)),
        acc: bigIntToPodI128(big(m.acc)),
        carry: bigIntToPodI128(big(m.carry)),
        accR: bigIntToPodU128(big(m.accR)),
        carryR: bigIntToPodU128(big(m.carryR)),
        dust: bigIntToPodI128(big(m.dust)),
        receivable: big(m.receivable),
        maxRelease: big(m.maxRelease),
        vestBal: big(m.vestBal),
        vestLast: big(m.vestLast),
        vestEnd: big(m.vestEnd),
        depositHoldUntil: big(m.depositHoldUntil ?? -1),
        shareEpoch: Number(m.shareEpoch),
        flags: Number(m.flags),
        venue,
        // pool_model has no venue-reward bit (`rewards_required = false`): no pump UVA.
        pumpUva: NO_UVA,
    };
}

export function vaultOf(v: VectorCase['vault']): VaultPoolFields {
    const lockCal = Array.from({ length: 128 }, (_, i) => bigIntToPodU128(big(v.lockCal[String(i)] ?? '0')));
    return {
        lenderSol: big(v.lenderSol),
        unmatched: big(v.unmatched),
        protocolFees: big(v.protocolFees),
        pi: bigIntToPodU128(big(v.pi)),
        lockNorm: bigIntToPodU128(big(v.lockNorm)),
        bMin: big(v.bMin),
        piEpoch: v.piEpoch,
        calDay: big(v.calDay),
        lockCal,
    };
}

export function lenderOf(l: Record<string, string | number>): LenderFields {
    return {
        shares: big(l.shares),
        snap: big(l.snap),
        pending: big(l.pending),
        snapR: big(l.snapR),
        real: big(l.real),
        claimed: big(l.claimed),
        epoch: Number(l.epoch),
        lock: big(l.lock),
        lockPi: big(l.lockPi),
        lockEpoch: Number(l.lockEpoch),
        lockTerm: big(l.lockTerm),
        lockUntil: big(l.lockUntil),
        fwd: Number(l.fwd ?? 0),
        fwdEpoch: Number(l.fwdEpoch ?? 0),
        fwdShares: big(l.fwdShares),
        fwdLag: Number(l.fwdLag ?? 0),
    };
}

export function recordOf(r: Record<string, string | number>): RecordFields {
    return {
        epoch: Number(r.epoch),
        accEnd: big(r.accEnd),
        accREnd: big(r.accREnd),
        sOld: big(r.sOld),
        carry: big(r.carry),
        carryR: big(r.carryR),
        num: Number(r.num),
        denLog1000: Number(r.denLog1000),
        convRem: big(r.convRem),
    };
}

export function stateOf(c: VectorCase, who: string | null): LendState {
    const l = who && who !== 'new' ? c.lenders[who] : undefined;
    return {
        market: marketOf(c.market),
        vault: vaultOf(c.vault),
        lender: l ? lenderOf(l) : null,
        records: c.records.map(recordOf),
    };
}

/** The trader's pump UVA of a fixture market on a pump venue (any non-default key). */
export const PUMP_UVA = address('So11111111111111111111111111111111111111112');
/** `Market.pump_uva` of a market listed off pump (Raydium, the mock): unset. */
export const NO_UVA = address(ZERO);

/**
 * A simple pool: `T` free tokens, `B` borrowed, `S` shares, nothing accrued, no locks. On a pump
 * venue (1–3) the market has the trader's pump UVA: the curve's venue cache sets
 * `Market.pump_uva` (`pump_curve::write_cache`) and the PumpSwap cache sets it when it is unset
 * (`pump_amm::write_cache`, at a PumpSwap listing or graduation) — on a market listed on PumpSwap
 * the UVA holds no curve-era cashback. Elsewhere none (a Raydium or mock listing), unless `pumpUva`
 * gives one (a curve market that left pump, AIR-05).
 */
export function simplePool(
    o: {
        tokensFree?: bigint;
        borrowedQ?: bigint;
        totalShares?: bigint;
        vestBal?: bigint;
        vestLast?: bigint;
        vestEnd?: bigint;
        depositHoldUntil?: bigint;
        flags?: number;
        venue?: number;
        pumpUva?: Address;
        shareEpoch?: number;
        lenderSol?: bigint;
        unmatched?: bigint;
        maxRelease?: bigint;
        receivable?: bigint;
    } = {},
): Pick<LendState, 'market' | 'vault'> {
    const venue = o.venue ?? 0;
    return {
        market: {
            tokensFree: o.tokensFree ?? 1_000_000_000n,
            borrowedQ: o.borrowedQ ?? 0n,
            totalShares: bigIntToPodU128(o.totalShares ?? 1_000_000_000_000n),
            acc: bigIntToPodI128(0n),
            carry: bigIntToPodI128(0n),
            accR: bigIntToPodU128(0n),
            carryR: bigIntToPodU128(0n),
            dust: bigIntToPodI128(0n),
            receivable: o.receivable ?? 0n,
            maxRelease: o.maxRelease ?? 0n,
            vestBal: o.vestBal ?? 0n,
            vestLast: o.vestLast ?? 0n,
            vestEnd: o.vestEnd ?? 0n,
            depositHoldUntil: o.depositHoldUntil ?? -1n,
            shareEpoch: o.shareEpoch ?? 0,
            flags: o.flags ?? MARKET_FLAG.wipeTraderSwept | MARKET_FLAG.wipeRewardsClaimed,
            venue,
            pumpUva: o.pumpUva ?? (venue >= 1 && venue <= 3 ? PUMP_UVA : NO_UVA),
        },
        vault: {
            lenderSol: o.lenderSol ?? 0n,
            unmatched: o.unmatched ?? 0n,
            protocolFees: 0n,
            pi: bigIntToPodU128(WAD),
            lockNorm: bigIntToPodU128(0n),
            bMin: 0n,
            piEpoch: 0,
            calDay: 1n,
            lockCal: Array.from({ length: 128 }, () => bigIntToPodU128(0n)),
        },
    };
}

/** A lender holding `shares` with nothing pending, no lock, in `epoch`. */
export function simpleLender(shares: bigint, o: Partial<LenderFields> = {}): LenderFields {
    return {
        shares,
        snap: 0n,
        pending: 0n,
        snapR: 0n,
        real: 0n,
        claimed: 0n,
        epoch: 0,
        lock: 0n,
        lockPi: WAD,
        lockEpoch: NO_LOCK_EPOCH,
        lockTerm: 0n,
        lockUntil: 0n,
        fwd: 0,
        fwdEpoch: 0,
        fwdShares: 0n,
        fwdLag: 0,
        ...o,
    };
}

/** A `Market` with the fields the lend panel reads (the rest is never read by it). */
export function fakeMarket(
    pool: Parameters<typeof simplePool>[0] = {},
    o: Partial<
        Pick<
            Market,
            | 'decimals'
            | 'tier'
            | 'lastAccrualTs'
            | 'tokenProgram'
            | 'mint'
            | 'marketVault'
            | 'poolVault'
            | 'trader'
            | 'traderWsol'
            | 'traderWsolAta'
            | 'traderBaseAta'
            | 'pumpUva'
            | 'poolCache'
            | 'emaPrice'
            | 'rentBasis'
            | 'rentAggr'
            | 'lenderCreditRem'
            | 'utilKinkBps'
        >
    > = {},
): Market {
    return {
        ...simplePool(pool).market,
        decimals: 6,
        tier: 1,
        lastAccrualTs: 0n,
        tokenProgram: address('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA'),
        mint: address('EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v'),
        marketVault: address('Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB'),
        poolVault: address('4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R'),
        trader: address('7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE'),
        traderWsol: address('mSoLzYCxHdYgdzU16g5QSh3i5K3z3KZK7ytfqcJm7So'),
        traderWsolAta: address(ZERO),
        traderBaseAta: address('JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN'),
        poolCache: Array.from({ length: 10 }, () => address(ZERO)),
        // No open position: nothing accrues.
        rentBasis: [0n, 0n, 0n].map(bigIntToPodU128),
        rentAggr: [0n, 0n, 0n].map(bigIntToPodU128),
        lenderCreditRem: bigIntToPodU128(0n),
        utilKinkBps: 8_000,
        ...o,
    } as unknown as Market;
}

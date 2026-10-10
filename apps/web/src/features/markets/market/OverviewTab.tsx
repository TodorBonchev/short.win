/**
 * The market page's Overview tab (FRONTEND.md §3.4): the market's accounts and state, fee lines,
 * exit counts and rates, realised lender loss by settlement condition, the creator-wallet deposits
 * (R1) and the venue trust assumptions (collapsed). From the chain alone (indexer down or unaware)
 * it shows the chain's state; history and analytics wait for the indexer.
 *
 * Layout (M10-RESP D10): hairline key / value blocks in two columns sharing the card's width from
 * 640 px, one on a phone, Fees under both; zeros and dashes muted; an all-zero settlements block is
 * one line ("No settlements yet") that opens to the figures. The fee lines are a DataTable (cards
 * when narrow).
 */
import type { Address } from '@solana/kit';
import { big, type FeeType, type MarketDetail } from '../../../api/types.ts';
import { AddressText } from '../../../components/Address.tsx';
import { Badge, venueLabel } from '../../../components/Badge.tsx';
import { type Column, DataTable } from '../../../components/DataTable.tsx';
import { Disclosure } from '../../../components/Disclosure.tsx';
import { Bps, Pnl, Sol, Tokens } from '../../../components/Num.tsx';
import { T } from '../../../config/testids.ts';
import { formatDuration } from '../../../lib/format.ts';
import { formatUtc } from '../../../lib/time.ts';
import { KvLines } from '../shared/KvLines.tsx';
import { CreatorDeposits } from './CreatorDeposits.tsx';
import type { MarketView } from './marketView.ts';

const FEE_TEXT: Readonly<Record<FeeType, string>> = {
    creation_fee: 'Market creation fee',
    open_fee: 'Open fees',
    venue_rent: 'Venue rent',
    rent_lender: 'Rent to lenders',
    rent_protocol: 'Rent to the protocol',
    liq_penalty_lender: 'Liquidation penalty to lenders',
    liq_penalty_protocol: 'Liquidation penalty to the protocol',
    liq_penalty_keeper: 'Liquidation penalty to keepers',
    liq_keeper_topup: 'Keeper top-up',
    expiry_fee: 'Expiry fees',
    expiry_keeper_reward: 'Expiry keeper rewards',
    settle_keeper_reward: 'Settlement keeper rewards',
    trader_sweep_lenders: 'Swept to lenders',
    trader_sweep_wipe: 'Swept to the wipe record',
    trader_sweep_protocol: 'Swept to the protocol',
    venue_rewards_lenders: 'Venue rewards to lenders',
    venue_rewards_wipe: 'Venue rewards to the wipe record',
    venue_rewards_protocol: 'Venue rewards to the protocol',
};

const ts = (v: number | null) => (v === null || v <= 0 ? '—' : formatUtc(v));
const bpsOrDash = (v: number | null) => (v === null ? <span className="muted">—</span> : <Bps value={v} />);
/** An amount row: muted when zero. */
const sol = (label: string, lamports: bigint) =>
    [label, <Sol key={label} lamports={lamports} />, { zero: lamports === 0n }] as const;
const pnl = (label: string, lamports: bigint) =>
    [label, <Pnl key={label} lamports={lamports} />, { zero: lamports === 0n }] as const;
const bpsRow = (label: string, v: number | null) => [label, bpsOrDash(v), { zero: v === null || v === 0 }] as const;

function MarketState({ view }: { view: MarketView }) {
    const d = view.detail;
    const decimals = view.decimals ?? 0;
    return (
        <section aria-label="Market state">
            <h3 className="mk-section-title">Market</h3>
            <KvLines
                rows={[
                    ['Venue', venueLabel(view.venue)],
                    ['Tier', `Tier ${view.tier}`],
                    ['Listed', ts(view.createdTs)],
                    ['Pool age from', ts(view.graduatedAt)],
                    [
                        'Token program',
                        view.tokenProgram ? <AddressText key="r58" address={view.tokenProgram} copy={false} /> : '—',
                    ],
                    [
                        'Tokens free (T)',
                        <Tokens key="t" raw={view.tokensFree} decimals={decimals} />,
                        { zero: view.tokensFree === 0n },
                    ],
                    [
                        'Tokens borrowed (ΣQ)',
                        <Tokens key="q" raw={view.borrowedQ} decimals={decimals} />,
                        { zero: view.borrowedQ === 0n },
                    ],
                    ['Open positions', String(view.openPositions)],
                    sol('Notional open', view.totalNotionalOpen),
                    ...(d
                        ? ([
                              sol('Collateral in open vaults', big(d.open_vault_lamports)),
                              sol('Lender SOL (vesting and claimable)', big(d.lender_sol)),
                              sol('Lender value', big(d.lender_value_lamports)),
                              sol('Inventory value', big(d.inventory_value_lamports)),
                              sol('Protocol fees (unswept)', big(d.protocol_fees)),
                              ['Share epoch', String(d.share_epoch)],
                              [
                                  'No route since',
                                  <span
                                      key="nr"
                                      data-testid={T.marketRouteChain('no-route')}
                                      data-value={d.route_chains.no_route_since}
                                  >
                                      {d.route_chains.no_route_since > 0 ? (
                                          <Badge tone="danger">{ts(d.route_chains.no_route_since)}</Badge>
                                      ) : (
                                          '—'
                                      )}
                                  </span>,
                                  { zero: d.route_chains.no_route_since <= 0 },
                              ],
                              [
                                  'Pool failing since',
                                  <span
                                      key="pf2"
                                      data-testid={T.marketRouteChain('pool-fail')}
                                      data-value={d.route_chains.pool_fail_since}
                                  >
                                      {d.route_chains.pool_fail_since > 0 ? (
                                          <Badge tone="warn">{ts(d.route_chains.pool_fail_since)}</Badge>
                                      ) : (
                                          '—'
                                      )}
                                  </span>,
                                  { zero: d.route_chains.pool_fail_since <= 0 },
                              ],
                          ] as const)
                        : []),
                ]}
            />
            {d && d.quarantined_events > 0 && (
                <p className="mk-note" role="note">
                    {d.quarantined_events} event(s) of this market could not be decoded by the indexer; its figures may
                    be incomplete.
                </p>
            )}
        </section>
    );
}

type FeeLine = MarketDetail['fees'][number];

const FEE_COLUMNS: readonly Column<FeeLine>[] = [
    { key: 'fee', header: 'Fee', card: 'title', render: (f) => FEE_TEXT[f.fee_type] ?? f.fee_type },
    {
        key: 'total',
        header: 'Total',
        align: 'right',
        card: 'aside',
        render: (f) => <Sol lamports={big(f.total)} />,
    },
    { key: 'to', header: 'To', render: (f) => <span className="secondary">{f.recipient}</span> },
    { key: 'n', header: 'Count', align: 'right', render: (f) => <span className="num">{f.n}</span> },
    { key: 'day', header: '24 h', align: 'right', render: (f) => <Sol lamports={big(f.last_24h)} /> },
];

function Fees({ d }: { d: MarketDetail }) {
    if (d.fees.length === 0) return <p className="muted">No fees yet.</p>;
    return (
        <DataTable
            caption="Fee lines"
            testId={T.marketFees}
            className="mk-tbl mk-fees-tbl"
            columns={FEE_COLUMNS}
            rows={d.fees}
            rowKey={(f) => `${f.fee_type}-${f.recipient}`}
        />
    );
}

function Exits({ d }: { d: MarketDetail }) {
    const e = d.exits;
    return (
        <section aria-label="Exits">
            <h3 className="mk-section-title">Exits</h3>
            <KvLines
                rows={[
                    ['Positions (open)', `${e.positions} (${e.open})`],
                    ['Closed by the borrower', String(e.closes)],
                    bpsRow('Close rate', e.close_rate_bps),
                    ['Liquidated (CR)', String(e.liquidations_cr)],
                    ['Liquidated after expiry', String(e.liquidations_expired)],
                    ['Expired', String(e.expiries)],
                    [
                        'Settled in SOL (S1 / S2 / S3)',
                        `${e.settlements_s1} / ${e.settlements_s2} / ${e.settlements_s3}`,
                    ],
                    bpsRow('Liquidation rate', e.liquidation_rate_bps),
                    bpsRow('Expiry rate', e.expiry_rate_bps),
                    bpsRow('Settlement rate', e.settlement_rate_bps),
                    ['Average duration', e.avg_duration_s === null ? '—' : formatDuration(e.avg_duration_s)],
                    bpsRow('Average open impact', e.avg_open_impact_bps),
                    bpsRow('Average exit slippage', e.avg_exit_slippage_bps),
                    pnl('Borrowers’ P&L', big(e.borrower_pnl)),
                ]}
            />
        </section>
    );
}

function LenderLoss({ d }: { d: MarketDetail }) {
    const l = d.lender_loss;
    const amounts = [
        l.token_value,
        l.lender_amount,
        l.lender_pnl,
        l.realised_lender_loss,
        l.realised_lender_loss_s1,
        l.realised_lender_loss_s2,
        l.realised_lender_loss_s3,
    ].map(big);
    const none = l.settlements === 0 && amounts.every((a) => a === 0n);
    const figures = (
        <KvLines
            rows={[
                ['Settlements', String(l.settlements)],
                sol('Token value owed', big(l.token_value)),
                sol('Paid to lenders', big(l.lender_amount)),
                pnl('Lenders’ P&L', big(l.lender_pnl)),
                sol('Realised loss', big(l.realised_lender_loss)),
                sol('  S1 — jumped past the vault', big(l.realised_lender_loss_s1)),
                sol('  S2 — buyback above max slippage', big(l.realised_lender_loss_s2)),
                sol('  S3 — no route', big(l.realised_lender_loss_s3)),
                bpsRow('Loss / token value', l.loss_bps_of_value),
            ]}
        />
    );
    return (
        <section
            aria-label="Realised lender loss"
            data-testid={T.marketLenderLoss}
            data-value={big(l.realised_lender_loss).toString()}
        >
            <h3 className="mk-section-title">Lenders’ SOL settlements</h3>
            {none ? (
                <details className="mk-none">
                    <summary>No settlements yet</summary>
                    {figures}
                </details>
            ) : (
                figures
            )}
        </section>
    );
}

export function OverviewTab({ mint, view }: { mint: Address; view: MarketView }) {
    const d = view.detail;
    return (
        <>
            <div className="mk-overview">
                <MarketState view={view} />
                {d ? (
                    <div className="mk-overview-col">
                        <Exits d={d} />
                        <LenderLoss d={d} />
                    </div>
                ) : (
                    <p className="muted">History and analytics return with the indexer.</p>
                )}
                {d && (
                    <section aria-label="Fees" className="mk-overview-fees">
                        <h3 className="mk-section-title">Fees</h3>
                        <Fees d={d} />
                    </section>
                )}
            </div>
            <CreatorDeposits mint={mint} detail={d} />
            <Disclosure id="market.venueTrust" collapsible />
        </>
    );
}

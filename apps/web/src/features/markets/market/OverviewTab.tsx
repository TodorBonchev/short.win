/**
 * The market page's Overview tab (FRONTEND.md §3.4): the market's accounts and state, fee lines,
 * exit counts and rates, realised lender loss by settlement condition, the creator-wallet deposits
 * (R1) and the venue trust assumptions (collapsed). From the chain alone (indexer down or unaware)
 * it shows the chain's state; history and analytics wait for the indexer.
 */
import type { Address } from '@solana/kit';
import { big, type FeeType, type MarketDetail } from '../../../api/types.ts';
import { AddressText } from '../../../components/Address.tsx';
import { Badge, venueLabel } from '../../../components/Badge.tsx';
import { Disclosure } from '../../../components/Disclosure.tsx';
import { Bps, Pnl, Sol, Tokens } from '../../../components/Num.tsx';
import { Kv } from '../../../components/States.tsx';
import { T } from '../../../config/testids.ts';
import { formatDuration } from '../../../lib/format.ts';
import { formatUtc } from '../../../lib/time.ts';
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

function MarketState({ view }: { view: MarketView }) {
    const d = view.detail;
    const decimals = view.decimals ?? 0;
    return (
        <section aria-label="Market state">
            <h3 className="mk-section-title">Market</h3>
            <Kv
                rows={[
                    ['Venue', venueLabel(view.venue)],
                    ['Tier', `Tier ${view.tier}`],
                    ['Listed', ts(view.createdTs)],
                    ['Pool age from', ts(view.graduatedAt)],
                    [
                        'Token program',
                        view.tokenProgram ? <AddressText key="r58" address={view.tokenProgram} copy={false} /> : '—',
                    ],
                    ['Tokens free (T)', <Tokens key="t" raw={view.tokensFree} decimals={decimals} />],
                    ['Tokens borrowed (ΣQ)', <Tokens key="q" raw={view.borrowedQ} decimals={decimals} />],
                    ['Open positions', String(view.openPositions)],
                    ['Notional open', <Sol key="n" lamports={view.totalNotionalOpen} />],
                    ...(d
                        ? ([
                              ['Collateral in open vaults', <Sol key="v" lamports={big(d.open_vault_lamports)} />],
                              ['Lender SOL (vesting and claimable)', <Sol key="ls" lamports={big(d.lender_sol)} />],
                              ['Lender value', <Sol key="lv" lamports={big(d.lender_value_lamports)} />],
                              ['Inventory value', <Sol key="iv" lamports={big(d.inventory_value_lamports)} />],
                              ['Protocol fees (unswept)', <Sol key="pf" lamports={big(d.protocol_fees)} />],
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

function Fees({ d }: { d: MarketDetail }) {
    if (d.fees.length === 0) return <p className="muted">No fees yet.</p>;
    return (
        <table className="tbl" data-testid={T.marketFees}>
            <caption className="visually-hidden">Fee lines</caption>
            <thead>
                <tr>
                    <th scope="col">Fee</th>
                    <th scope="col">To</th>
                    <th scope="col" data-align="right">
                        Count
                    </th>
                    <th scope="col" data-align="right">
                        Total
                    </th>
                    <th scope="col" data-align="right">
                        24 h
                    </th>
                </tr>
            </thead>
            <tbody>
                {d.fees.map((f) => (
                    <tr key={`${f.fee_type}-${f.recipient}`}>
                        <td>{FEE_TEXT[f.fee_type] ?? f.fee_type}</td>
                        <td className="secondary">{f.recipient}</td>
                        <td data-align="right" className="num">
                            {f.n}
                        </td>
                        <td data-align="right">
                            <Sol lamports={big(f.total)} />
                        </td>
                        <td data-align="right">
                            <Sol lamports={big(f.last_24h)} />
                        </td>
                    </tr>
                ))}
            </tbody>
        </table>
    );
}

function Exits({ d }: { d: MarketDetail }) {
    const e = d.exits;
    return (
        <section aria-label="Exits">
            <h3 className="mk-section-title">Exits</h3>
            <Kv
                rows={[
                    ['Positions (open)', `${e.positions} (${e.open})`],
                    ['Closed by the borrower', String(e.closes)],
                    ['Close rate', bpsOrDash(e.close_rate_bps)],
                    ['Liquidated (CR)', String(e.liquidations_cr)],
                    ['Liquidated after expiry', String(e.liquidations_expired)],
                    ['Expired', String(e.expiries)],
                    [
                        'Settled in SOL (S1 / S2 / S3)',
                        `${e.settlements_s1} / ${e.settlements_s2} / ${e.settlements_s3}`,
                    ],
                    ['Liquidation rate', bpsOrDash(e.liquidation_rate_bps)],
                    ['Expiry rate', bpsOrDash(e.expiry_rate_bps)],
                    ['Settlement rate', bpsOrDash(e.settlement_rate_bps)],
                    ['Average duration', e.avg_duration_s === null ? '—' : formatDuration(e.avg_duration_s)],
                    ['Average open impact', bpsOrDash(e.avg_open_impact_bps)],
                    ['Average exit slippage', bpsOrDash(e.avg_exit_slippage_bps)],
                    ['Borrowers’ P&L', <Pnl key="p" lamports={big(e.borrower_pnl)} />],
                ]}
            />
        </section>
    );
}

function LenderLoss({ d }: { d: MarketDetail }) {
    const l = d.lender_loss;
    return (
        <section
            aria-label="Realised lender loss"
            data-testid={T.marketLenderLoss}
            data-value={big(l.realised_lender_loss).toString()}
        >
            <h3 className="mk-section-title">Lenders’ SOL settlements</h3>
            <Kv
                rows={[
                    ['Settlements', String(l.settlements)],
                    ['Token value owed', <Sol key="tv" lamports={big(l.token_value)} />],
                    ['Paid to lenders', <Sol key="la" lamports={big(l.lender_amount)} />],
                    ['Lenders’ P&L', <Pnl key="lp" lamports={big(l.lender_pnl)} />],
                    ['Realised loss', <Sol key="rl" lamports={big(l.realised_lender_loss)} />],
                    ['  S1 — jumped past the vault', <Sol key="s1" lamports={big(l.realised_lender_loss_s1)} />],
                    ['  S2 — buyback above max slippage', <Sol key="s2" lamports={big(l.realised_lender_loss_s2)} />],
                    ['  S3 — no route', <Sol key="s3" lamports={big(l.realised_lender_loss_s3)} />],
                    ['Loss / token value', bpsOrDash(l.loss_bps_of_value)],
                ]}
            />
        </section>
    );
}

export function OverviewTab({ mint, view }: { mint: Address; view: MarketView }) {
    const d = view.detail;
    return (
        <>
            <div className="mk-grid-2">
                <MarketState view={view} />
                {d ? <Exits d={d} /> : <p className="muted">History and analytics return with the indexer.</p>}
            </div>
            {d && (
                <div className="mk-grid-2">
                    <section aria-label="Fees">
                        <h3 className="mk-section-title">Fees</h3>
                        <Fees d={d} />
                    </section>
                    <LenderLoss d={d} />
                </div>
            )}
            <CreatorDeposits mint={mint} detail={d} />
            <Disclosure id="market.venueTrust" collapsible />
        </>
    );
}

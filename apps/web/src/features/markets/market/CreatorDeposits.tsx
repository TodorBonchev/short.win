/**
 * Creator-wallet deposits (SPEC §7, §8, R1; FRONTEND.md §3.4): the coin creator's exposure as the
 * indexer measures it (its share of the pool, tokens deposited, the creator fee it earned on
 * short.win trades) and every lender row from the coin creator's or the market creator's wallet,
 * next to the `market.creatorSelfDealing` disclosure. Wallets the creator controls but did not use
 * for the coin cannot be identified; the disclosure says so.
 */
import type { Address } from '@solana/kit';
import { useQuery } from '@tanstack/react-query';
import { indexerQueries } from '../../../api/queries.ts';
import { big, type LenderPosition, type MarketDetail } from '../../../api/types.ts';
import { AddressText } from '../../../components/Address.tsx';
import { Badge } from '../../../components/Badge.tsx';
import { type Column, DataTable } from '../../../components/DataTable.tsx';
import { Disclosure } from '../../../components/Disclosure.tsx';
import { Term } from '../../../components/Glossary.tsx';
import { Bps, Sol, Tokens } from '../../../components/Num.tsx';
import { Skeleton } from '../../../components/States.tsx';
import { T } from '../../../config/testids.ts';
import { formatSol } from '../../../lib/format.ts';
import { KvLines } from '../shared/KvLines.tsx';

export function CreatorBadges({ l }: { l: Pick<LenderPosition, 'is_coin_creator' | 'is_market_creator'> }) {
    if (!l.is_coin_creator && !l.is_market_creator) return null;
    return (
        <span className="mk-badges">
            {l.is_coin_creator && (
                <Term id="coinCreator">
                    <Badge tone="warn" title="This wallet created the token (SPEC R1)">
                        Coin creator
                    </Badge>
                </Term>
            )}
            {l.is_market_creator && (
                <Term id="marketCreator">
                    <Badge tone="info" title="This wallet created the short.win market">
                        Market creator
                    </Badge>
                </Term>
            )}
        </span>
    );
}

function creatorColumns(decimals: number): Column<LenderPosition>[] {
    return [
        {
            key: 'wallet',
            header: 'Wallet',
            card: 'title',
            render: (l) => (
                <>
                    <AddressText address={l.owner} />
                    <CreatorBadges l={l} />
                </>
            ),
        },
        {
            key: 'share',
            header: 'Share',
            align: 'right',
            card: 'aside',
            render: (l) => (l.share_bps === null ? '—' : <Bps value={l.share_bps} />),
        },
        {
            key: 'deposited',
            header: 'Deposited',
            align: 'right',
            render: (l) => <Tokens raw={big(l.deposited_tokens)} decimals={decimals} />,
        },
        {
            key: 'withdrawn',
            header: 'Withdrawn',
            align: 'right',
            render: (l) => <Tokens raw={big(l.withdrawn_tokens)} decimals={decimals} />,
        },
    ];
}

export function CreatorDeposits({ mint, detail }: { mint: Address; detail: MarketDetail | null }) {
    const lenders = useQuery({ ...indexerQueries.marketLenders(mint), enabled: detail !== null });
    const decimals = detail?.decimals ?? 0;
    const exp = detail?.creator_exposure ?? null;
    const rows = (lenders.data?.data.data ?? []).filter((l) => l.is_coin_creator || l.is_market_creator);
    const creatorFee = exp ? `${formatSol(big(exp.creator_fee_income_lamports), 4)} SOL` : 'n/a (indexer unavailable)';
    return (
        <section className="mk-creator" data-testid={T.marketCreatorDeposits} aria-label="Creator-wallet deposits">
            <h3>Creator-wallet deposits</h3>
            {exp ? (
                <KvLines
                    rows={[
                        [
                            'Coin creator',
                            exp.coin_creator ? (
                                <AddressText key="r53" address={exp.coin_creator} />
                            ) : (
                                <span key="r55" className="muted">
                                    Unknown
                                </span>
                            ),
                        ],
                        [
                            'Its share of the pool',
                            exp.coin_creator_share_bps === null ? (
                                <span key="r61" className="muted">
                                    —
                                </span>
                            ) : (
                                <Bps key="r63" value={exp.coin_creator_share_bps} />
                            ),
                            { zero: !exp.coin_creator_share_bps },
                        ],
                        [
                            'Tokens it deposited',
                            <Tokens
                                key="t"
                                raw={big(exp.coin_creator_deposited_tokens)}
                                decimals={decimals}
                                symbol={detail?.symbol}
                            />,
                            { zero: big(exp.coin_creator_deposited_tokens) === 0n },
                        ],
                        [
                            'Creator fee from short.win trades',
                            <Sol key="f" lamports={big(exp.creator_fee_income_lamports)} />,
                            { zero: big(exp.creator_fee_income_lamports) === 0n },
                        ],
                        ['Market created by', detail ? <AddressText key="r79" address={detail.creator} /> : '—'],
                    ]}
                />
            ) : (
                <p className="muted">Creator exposure needs the indexer, which is unavailable.</p>
            )}
            {detail && (
                <div>
                    {lenders.isPending ? (
                        <Skeleton width={160} />
                    ) : rows.length === 0 ? (
                        <p className="muted">
                            No deposits from the coin creator’s or the market creator’s wallet among the largest
                            lenders.
                        </p>
                    ) : (
                        <DataTable
                            caption="Deposits from creator wallets"
                            className="mk-tbl mk-creator-tbl"
                            columns={creatorColumns(decimals)}
                            rows={rows}
                            rowKey={(l) => l.owner}
                        />
                    )}
                </div>
            )}
            <Disclosure id="market.creatorSelfDealing" vars={{ creatorFee }} />
        </section>
    );
}

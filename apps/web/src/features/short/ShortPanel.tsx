/**
 * SHORT track — the market page's Short tab (FRONTEND.md §3.7): size `Q`, slippage, the full open
 * preview (proceeds `S`, D4 margin, open fee, total cost, opening CR at spot and EMA, liquidation
 * price, rent rate and minimum rent, max duration and deadline, the tier terms locked at open, the
 * caps with the binding one marked), the refusal the program would return (blocked states before
 * any size; a cap above the binding size), the `borrower.liquidation` disclosure with its required
 * checkbox, then `open_short` through the transaction pipeline (simulated before the wallet is
 * asked, reviewed with its deltas; FE-D2).
 *
 * Every number is read from the chain (FE-D1; `hooks.ts`) and computed by the WASM facade
 * (`previews.ts`), refreshed every 2 s and on every input.
 */
import { getShortPositionSize, getWalletExposureSize } from '@shortwin/client';
import type { Address } from '@solana/kit';
import { Link } from '@tanstack/react-router';
import { type ReactNode, useEffect, useId, useMemo, useState } from 'react';
import { Button } from '../../components/Button.tsx';
import { Disclosure } from '../../components/Disclosure.tsx';
import { AmountInput, Field } from '../../components/Field.tsx';
import { FormFoot } from '../../components/FormFoot.tsx';
import { Term } from '../../components/Glossary.tsx';
import { Price, Sol, Tokens } from '../../components/Num.tsx';
import { usePageBanners } from '../../components/PageBanners.tsx';
import { ErrorState, Skeleton } from '../../components/States.tsx';
import { TokenId } from '../../components/TokenId.tsx';
import { routeBrokenVars } from '../../config/disclosureVars.ts';
import { glossaryFor } from '../../config/glossary.ts';
import { T } from '../../config/testids.ts';
import { parseAmount, parseBps } from '../../lib/amount.ts';
import { formatBps, formatDuration, formatTokens } from '../../lib/format.ts';
import { formatUtc } from '../../lib/time.ts';
import { useAppClient } from '../../rpc/context.tsx';
import { ReadAgeView, useReadAge } from '../../shell/SlotAge.tsx';
import { TxReviewModal } from '../../tx/TxReviewModal.tsx';
import { ConnectCta } from '../../wallet/ConnectCta.tsx';
import { useWallet } from '../../wallet/useWallet.ts';
import { CoverageLink } from './CoverageLink.tsx';
import { firstReason, REASON, useConfirmedToast } from './feedback.ts';
import { useExposureQ, useRentExempt, useShortSnapshot, useTokenLabel } from './hooks.ts';
import { borrowerDisclosureVars } from './plans.ts';
import {
    assignTier,
    type CapName,
    capsFor,
    DEFAULT_SLIPPAGE_BPS,
    MAX_SLIPPAGE_BPS,
    MIN_SLIPPAGE_BPS,
    openBlocked,
    previewOpen,
} from './previews.ts';
import { freshOpenTrade, openTrade } from './trades.ts';
import { BlockedNotice, CAP_TEXT, CrValue, errorByName, plainAmount, TierTerms } from './ui.tsx';
import { useTradeFlow } from './useTradeFlow.ts';
import './short.css';

function Row({
    label,
    sub,
    children,
    testId,
    value,
}: {
    label: string;
    sub?: ReactNode;
    children: ReactNode;
    testId: string;
    value?: string | undefined;
}) {
    return (
        <div style={{ display: 'contents' }}>
            <dt>
                <Term>{label}</Term>
            </dt>
            <dd data-testid={testId} data-value={value}>
                {children}
                {sub && <span className="short-preview-sub">{sub}</span>}
            </dd>
        </div>
    );
}

/** Why a size does not parse, in the words of the reason line. */
const SIZE_REASON = {
    empty: 'Enter a size',
    format: 'Enter the size as a plain number',
    decimals: 'The size has more decimals than the token',
    zero: 'Enter a size above zero',
    range: 'The size is too large',
} as const;

/** The reason line while the acknowledgement is what blocks the submit (a button to the box). */
const ACK_REASON = 'Tick “I understand…” above';

/** The reason line for a size the program would refuse: a cap to stay under, or the minimum. */
function refusalReason(refusal: string | null, cap: CapName | null): string {
    if (cap) return `Lower the size: it is above the ${CAP_TEXT[cap].toLowerCase()}`;
    if (refusal === 'BelowMinNotional') return 'Raise the size: it is below the minimum position value';
    return `The program would refuse this size (${refusal ?? 'unknown'})`;
}

/** The default slippage as the input shows it ("1"). */
const DEFAULT_SLIPPAGE_TEXT = String(DEFAULT_SLIPPAGE_BPS / 100);

/** A UTC time that may break between its date and its time, never inside either (a 320 px sheet). */
function UtcParts({ unix }: { unix: bigint }) {
    const text = formatUtc(unix);
    const at = text.indexOf(' ');
    return (
        <>
            <span className="num">{text.slice(0, at)}</span> <span className="num">{text.slice(at + 1)}</span>
        </>
    );
}

/** A token label whose shortened mint ("CMer…rrRZ", after the last " · ") never breaks at its "…". */
function keepMintWhole(label: string): ReactNode {
    const at = label.lastIndexOf(' · ');
    const mint = at < 0 ? label : label.slice(at + 3);
    return (
        <>
            {at < 0 ? null : label.slice(0, at + 3)}
            <span className="nowrap">{mint}</span>
        </>
    );
}

export function ShortPanel({ mint }: { mint: Address }) {
    const { connected } = useWallet();
    const owner = connected?.address ?? null;
    const pageBanners = usePageBanners();
    const snap = useShortSnapshot(mint);
    const s = snap.data ?? null;
    const math = snap.math;
    const exposure = useExposureQ(mint, owner);
    const walletQ = exposure.data ?? 0n;
    const token = useTokenLabel(mint);
    const symbol = token.symbol;
    /** The symbol with the short mint (M9-SEC-5): the panel names the token by its mint too. */
    const tokenText = token.label ?? symbol;
    const positionRent = useRentExempt(getShortPositionSize()).data ?? null;
    const exposureRent = useRentExempt(getWalletExposureSize()).data ?? null;
    const trade = useTradeFlow();
    const [qText, setQText] = useState('');
    const [slipText, setSlipText] = useState(DEFAULT_SLIPPAGE_TEXT);
    const [ack, setAck] = useState(false);
    const [lastPosition, setLastPosition] = useState<Address | null>(null);
    const [submitError, setSubmitError] = useState<string | null>(null);
    const client = useAppClient();
    // The age of the snapshot behind the preview (FRONTEND.md §6.6): shown; Submit waits when stale.
    const age = useReadAge(s?.slot ?? null, s?.fetchedAt ?? null);
    // The reason line's button focuses the acknowledgement box (FRONTEND.md §7.6).
    const ackId = useId();
    useConfirmedToast(trade.flow, () =>
        lastPosition ? [{ href: `/positions/${lastPosition}`, text: 'View position' }] : [],
    );

    // Acknowledgements are per transaction (§4.1 rule 3): a confirmed open clears the box.
    const flowStage = trade.flow.state.stage;
    useEffect(() => {
        if (flowStage === 'confirmed') setAck(false);
    }, [flowStage]);

    const decimals = s?.decimals ?? 0;
    const parsedQ = parseAmount(qText, decimals);
    const q = parsedQ.ok ? parsedQ.raw : null;
    const slip = parseBps(slipText, MAX_SLIPPAGE_BPS);
    const slipOk = slip !== null && slip >= MIN_SLIPPAGE_BPS;

    const view = useMemo(() => {
        if (!math || !s) return null;
        const blocked = openBlocked(math, s);
        const tier = assignTier(math, s);
        const tp = tier === null ? undefined : s.config.tiers[tier];
        const caps = tier === null || !tp ? null : capsFor(math, s, tp, walletQ);
        const preview =
            q !== null && slipOk && slip !== null ? previewOpen(math, s, { q, slippageBps: slip, walletQ }) : null;
        return { blocked, tier, tp, caps, preview };
    }, [math, s, q, slip, slipOk, walletQ]);

    if (snap.error || snap.mathError) {
        return (
            <div className="short-panel" data-testid={T.shortPanel} data-mint={mint}>
                <ErrorState error={snap.error ?? snap.mathError} retry={() => void snap.refetch()} />
            </div>
        );
    }
    if (snap.data === null) {
        return (
            <div className="short-panel" data-testid={T.shortPanel} data-mint={mint}>
                <div className="empty">This token has no short.win market yet.</div>
            </div>
        );
    }
    if (!s || !math || !view) {
        return (
            <div className="short-panel" data-testid={T.shortPanel} data-mint={mint} aria-busy="true">
                <Skeleton width={200} />
                <Skeleton width={260} />
                <Skeleton width={180} />
            </div>
        );
    }

    const { blocked, tier, tp, caps, preview } = view;
    const coverageLinked = blocked?.code === 'CoveragePaused' && !!pageBanners?.shown.has('coverage-paused');
    // A coverage pause replaces the form (§4 `market.coveragePause`); so does a list-only venue.
    const replaceForm = blocked?.code === 'CoveragePaused' || blocked?.code === 'NoSupportedVenue';
    const rents = (positionRent ?? 0n) + (walletQ > 0n ? 0n : (exposureRent ?? 0n));
    const totalCost = preview ? preview.margin + preview.openFee + rents : null;
    const refusalInfo = preview?.refusal ? errorByName(preview.refusal) : undefined;
    const stage = trade.flow.state.stage;
    // The submit's reason line and its enabled state are one predicate list (FRONTEND.md §7.6).
    const reason = firstReason([
        [!connected, REASON.connect],
        [!connected?.signer, REASON.signer],
        [!!blocked, 'New shorts are unavailable on this market now'],
        [!parsedQ.ok, parsedQ.ok ? '' : SIZE_REASON[parsedQ.error]],
        [!slipOk, REASON.slippage],
        [!preview, 'No quote for this size on the venue now'],
        [!!preview?.refusal, refusalReason(preview?.refusal ?? null, preview?.refusalCap ?? null)],
        [!ack, ACK_REASON],
        [age.state === 'stale', REASON.stale],
        [trade.building, REASON.building],
        [stage !== 'idle' && stage !== 'confirmed', REASON.inReview],
    ]);
    const canSubmit = reason === null;

    const submit = async () => {
        if (!preview || !connected?.signer || !tp || slip === null) return;
        const inputs = {
            q: preview.q,
            slippageBps: slip,
            signer: connected.signer,
            symbol,
            onPosition: setLastPosition,
        };
        setSubmitError(null);
        try {
            const t = await openTrade(math, s, walletQ, inputs);
            // Retry after a moved quote rebuilds the same open from a fresh read (M9-UX-1).
            trade.start(t.build, t.candidates, () => freshOpenTrade(client.rpc, math, mint, inputs));
        } catch (e) {
            setSubmitError(e instanceof Error ? e.message : String(e));
        }
    };

    const maxQ = caps?.maxQ ?? null;
    const capWarning = preview?.refusalCap ?? (preview?.refusal ? 'notional' : null);

    return (
        <div className="short-panel" data-testid={T.shortPanel} data-mint={mint}>
            {blocked && (
                <BlockedNotice
                    blocked={blocked}
                    vars={routeBrokenVars(s.config.params)}
                    instead={coverageLinked ? <CoverageLink /> : undefined}
                />
            )}
            {!replaceForm && (
                <>
                    <div className="short-inputs">
                        <AmountInput
                            label={
                                <>
                                    Size (<bdi className="case-keep">{keepMintWhole(tokenText)}</bdi>)
                                </>
                            }
                            value={qText}
                            onChange={(t) => setQText(t)}
                            decimals={decimals}
                            unit={symbol}
                            max={maxQ !== null && maxQ > 0n ? maxQ : undefined}
                            formatMax={(raw) => plainAmount(raw, decimals)}
                            testId={T.shortQInput}
                            hint={
                                maxQ !== null && caps ? (
                                    <>
                                        Max now <Tokens raw={maxQ} decimals={decimals} /> ({CAP_TEXT[caps.binding]})
                                    </>
                                ) : undefined
                            }
                        />
                        <Field label="Slippage" error={!slipOk ? 'Use 0.1–50%.' : null}>
                            {({ inputId, describedBy }) => (
                                <div className="input-wrap" data-invalid={!slipOk ? 'true' : undefined}>
                                    <input
                                        id={inputId}
                                        className="input num"
                                        inputMode="decimal"
                                        autoComplete="off"
                                        value={slipText}
                                        aria-invalid={!slipOk || undefined}
                                        aria-describedby={describedBy}
                                        data-testid={T.shortSlippageInput}
                                        data-value={slipOk ? String(slip) : undefined}
                                        onChange={(e) => setSlipText(e.target.value.replace(',', '.'))}
                                    />
                                    <span className="input-suffix">%</span>
                                </div>
                            )}
                        </Field>
                    </div>

                    <section className="short-preview" aria-label="Open preview">
                        <div className="short-preview-head">
                            <h3 className="short-section-title">
                                Preview{' '}
                                <span className="case-keep">
                                    <TokenId symbol={symbol} mint={mint} />
                                </span>
                            </h3>
                            <ReadAgeView age={age} />
                        </div>
                        {q === null || !preview ? (
                            <p className="short-note">
                                {q === null
                                    ? 'Enter a size to see the proceeds, margin, fee and risk of this short.'
                                    : 'No quote for this size on the venue now.'}
                            </p>
                        ) : (
                            <dl className="kv">
                                <Row label="Borrow" testId={T.shortPreview('q')} value={preview.q.toString()}>
                                    <Tokens raw={preview.q} decimals={decimals} symbol={symbol} />
                                </Row>
                                <Row
                                    label="Sale proceeds (S)"
                                    testId={T.shortPreview('proceeds')}
                                    value={preview.proceeds.toString()}
                                    sub={
                                        <>
                                            at least <Sol lamports={preview.minProceeds} unit={false} /> SOL
                                        </>
                                    }
                                >
                                    <Sol lamports={preview.proceeds} />
                                </Row>
                                <Row
                                    label="Margin (D4)"
                                    testId={T.shortPreview('margin')}
                                    value={preview.margin.toString()}
                                    sub={
                                        <>
                                            at most <Sol lamports={preview.maxMargin} unit={false} /> SOL
                                        </>
                                    }
                                >
                                    <Sol lamports={preview.margin} />
                                </Row>
                                <Row
                                    label="Open fee"
                                    testId={T.shortPreview('open-fee')}
                                    value={preview.openFee.toString()}
                                >
                                    <Sol lamports={preview.openFee} />
                                </Row>
                                <Row
                                    label="Total cost"
                                    testId={T.shortPreview('total-cost')}
                                    value={totalCost?.toString()}
                                    sub="margin, fee and account rent (rent returns on close); network fee in the review"
                                >
                                    {totalCost === null ? '—' : <Sol lamports={totalCost} />}
                                </Row>
                                <Row
                                    label="CR at open (spot)"
                                    testId={T.shortPreview('cr-open')}
                                    value={preview.crOpenBps.toString()}
                                >
                                    <CrValue bps={preview.crOpenBps} liqBps={preview.terms.crLiqBps} />
                                </Row>
                                <Row
                                    label="CR at open (EMA)"
                                    testId={T.shortPreview('cr-ema')}
                                    value={preview.crEmaBps?.toString()}
                                >
                                    <CrValue bps={preview.crEmaBps} liqBps={preview.terms.crLiqBps} />
                                </Row>
                                <Row
                                    label="Liquidation price"
                                    testId={T.shortPreview('liq-price')}
                                    value={preview.liqPriceWad?.toString()}
                                    sub={`where CR falls below ${formatBps(preview.terms.crLiqBps, 0)}`}
                                >
                                    {preview.liqPriceWad === null ? (
                                        '—'
                                    ) : (
                                        <Price wad={preview.liqPriceWad} decimals={decimals} />
                                    )}
                                </Row>
                                <Row
                                    label="Rent rate (APR)"
                                    testId={T.shortPreview('rent-rate')}
                                    value={String(preview.rentRateBps)}
                                    sub={
                                        <>
                                            ≈ <Sol lamports={preview.rentPerDay} frac={6} unit={false} /> SOL per day at
                                            the smoothed price
                                        </>
                                    }
                                >
                                    {formatBps(preview.rentRateBps)}
                                </Row>
                                <Row
                                    label="Minimum rent"
                                    testId={T.shortPreview('min-rent')}
                                    value={preview.minRent.toString()}
                                >
                                    <Sol lamports={preview.minRent} frac={6} />
                                </Row>
                                <Row
                                    label="Max duration"
                                    testId={T.shortPreview('max-duration')}
                                    value={String(preview.maxDurationS)}
                                >
                                    {formatDuration(preview.maxDurationS)}
                                </Row>
                                <Row
                                    label="Deadline"
                                    testId={T.shortPreview('deadline')}
                                    value={preview.deadline.toString()}
                                >
                                    <UtcParts unix={preview.deadline} />
                                </Row>
                            </dl>
                        )}
                        {caps && (
                            <dl
                                className="short-caps"
                                data-testid={T.shortPreview('caps')}
                                data-value={caps.maxQ.toString()}
                                data-cap={caps.binding}
                                aria-label="Size caps"
                            >
                                {(['position', 'market', 'utilisation', 'gamma', 'wallet', 'inventory'] as const).map(
                                    (k) => {
                                        const v = caps.max[k];
                                        if (v === null) return null;
                                        const binding = caps.binding === k ? 'true' : undefined;
                                        return (
                                            <div key={k} style={{ display: 'contents' }}>
                                                <dt data-binding={binding}>
                                                    {/* One string: a wrapped label keeps its ⓘ on its last word. */}
                                                    <Term id={glossaryFor(CAP_TEXT[k])}>
                                                        {`${CAP_TEXT[k]}${binding ? ' (binding)' : ''}`}
                                                    </Term>
                                                </dt>
                                                <dd className="num short-cap-value" data-binding={binding}>
                                                    {formatTokens(v, decimals)}
                                                </dd>
                                            </div>
                                        );
                                    },
                                )}
                            </dl>
                        )}
                    </section>

                    {capWarning && preview?.refusal && (
                        <div
                            className="banner"
                            data-tone="warn"
                            role="alert"
                            data-testid={T.shortCapWarning}
                            data-cap={capWarning}
                        >
                            <div>
                                <strong>{preview.refusalCap ? `${CAP_TEXT[capWarning]}.` : 'Size refused.'}</strong>{' '}
                                {refusalInfo?.message ?? preview.refusal}
                            </div>
                        </div>
                    )}

                    {tier !== null && tp && <TierTerms tier={tier} tp={tp} params={s.config.params} />}

                    {tp && (
                        <Disclosure
                            id="borrower.liquidation"
                            vars={borrowerDisclosureVars(tp, s.config.params)}
                            acknowledged={ack}
                            onAcknowledge={setAck}
                            ackId={ackId}
                        />
                    )}
                    {tp && (
                        <Disclosure
                            id="borrower.residuals"
                            vars={borrowerDisclosureVars(tp, s.config.params)}
                            collapsible
                        />
                    )}

                    <FormFoot
                        reason={reason}
                        reasonTestId={T.shortSubmitReason}
                        reasonTarget={reason === ACK_REASON && tp ? ackId : undefined}
                    >
                        {(trade.buildError ?? submitError) && (
                            <div className="banner" data-tone="danger" role="alert">
                                {trade.buildError ?? submitError}
                            </div>
                        )}
                        {!connected && <ConnectCta surface="short" />}
                        <Button
                            variant="short"
                            block
                            disabled={!canSubmit}
                            busy={trade.building}
                            onClick={() => void submit()}
                            data-testid={T.shortSubmit}
                        >
                            {!connected ? 'Connect a wallet to short' : `Short ${tokenText}`}
                        </Button>
                    </FormFoot>
                    {trade.fallback && (
                        <p className="short-note" role="status">
                            The venue’s extra accounts changed; retried with the {trade.fallback} set.
                        </p>
                    )}
                    {stage === 'confirmed' && lastPosition && (
                        <p className="short-note" role="status">
                            Position opened.{' '}
                            <Link to="/positions/$position" params={{ position: lastPosition }}>
                                View position
                            </Link>
                        </p>
                    )}
                </>
            )}
            <TxReviewModal flow={trade.flow} />
        </div>
    );
}

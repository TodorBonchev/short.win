/**
 * Close a position (FRONTEND.md §3.8, §5.10): the buyback at the live quote, the rent due, what
 * comes back and the P&L, `close_short(max_buyback_cost = ⌈B̂_live·(1 + slippage)⌉)` through the
 * pipeline. A close that would refuse says why before anything is signed: `InsufficientVault`
 * (add margin first), no route. A close past a live curve's `rT` is an ordinary close at `B̂_v3`:
 * it completes the curve by pump's synthetic migration in the same instruction (pump-final §5.2,
 * §8; FE-3 closed), so it has no notice of its own.
 */
import type { ShortPosition } from '@shortwin/client';
import type { Address } from '@solana/kit';
import { useState } from 'react';
import { Button } from '../../../components/Button.tsx';
import { Field } from '../../../components/Field.tsx';
import { FormFoot } from '../../../components/FormFoot.tsx';
import { Pnl, Sol, Tokens } from '../../../components/Num.tsx';
import { T } from '../../../config/testids.ts';
import { parseBps } from '../../../lib/amount.ts';
import type { MathModule } from '../../../math/load.ts';
import { useAppClient } from '../../../rpc/context.tsx';
import { TxReviewModal } from '../../../tx/TxReviewModal.tsx';
import { useWallet } from '../../../wallet/useWallet.ts';
import { firstReason, REASON, useConfirmedToast } from '../feedback.ts';
import {
    DEFAULT_SLIPPAGE_BPS,
    MAX_SLIPPAGE_BPS,
    MIN_SLIPPAGE_BPS,
    positionMetrics,
    previewCloseFor,
} from '../previews.ts';
import type { ChainSnapshot } from '../snapshot.ts';
import { closeTrade, freshCloseTrade } from '../trades.ts';
import { errorByName } from '../ui.tsx';
import { useTradeFlow } from '../useTradeFlow.ts';

export function CloseForm({
    math,
    snapshot,
    position,
    pos,
    symbol,
    stale = false,
}: {
    math: MathModule;
    snapshot: ChainSnapshot;
    position: Address;
    pos: ShortPosition;
    symbol: string;
    /** The snapshot behind the preview is stale (FRONTEND.md §6.6): Submit waits for a fresh read. */
    stale?: boolean;
}) {
    const { connected } = useWallet();
    const client = useAppClient();
    const trade = useTradeFlow();
    const [submitError, setSubmitError] = useState<string | null>(null);
    const [slipText, setSlipText] = useState(String(DEFAULT_SLIPPAGE_BPS / 100));
    const slip = parseBps(slipText, MAX_SLIPPAGE_BPS);
    const slipOk = slip !== null && slip >= MIN_SLIPPAGE_BPS;
    const mx = positionMetrics(math, snapshot, pos);
    const preview = slipOk && slip !== null ? previewCloseFor(math, snapshot, pos, slip) : null;
    const owner = connected?.address;
    const mine = owner === pos.borrower;
    const venueCannot = snapshot.kind === 'pump_migrating' || snapshot.kind === 'none' || !snapshot.venue.executable;
    const stage = trade.flow.state.stage;
    // The reason line and the enabled state are one predicate list (FRONTEND.md §7.6).
    const reason = firstReason([
        [!connected, REASON.connect],
        [!connected?.signer, REASON.signer],
        [!mine, REASON.notMine],
        [!slipOk, REASON.slippage],
        [
            !preview,
            venueCannot
                ? 'The venue cannot trade now'
                : mx.insufficientVault
                  ? 'Add margin first: the vault cannot cover the buyback'
                  : 'No buyback quote now',
        ],
        [stale, REASON.stale],
        [trade.building, REASON.building],
        [stage !== 'idle' && stage !== 'confirmed', REASON.inReview],
    ]);
    const canSubmit = reason === null;
    useConfirmedToast(trade.flow);

    const submit = async () => {
        if (!preview || !connected?.signer || slip === null) return;
        const inputs = { position, slippageBps: slip, signer: connected.signer, symbol };
        setSubmitError(null);
        try {
            const t = await closeTrade(math, snapshot, pos, inputs);
            // Retry after a moved quote rebuilds the same close from a fresh read (M9-UX-1).
            trade.start(t.build, t.candidates, () => freshCloseTrade(client.rpc, math, snapshot.mint, inputs));
        } catch (e) {
            setSubmitError(e instanceof Error ? e.message : String(e));
        }
    };

    return (
        <section className="stack" id="close" aria-labelledby="close-title">
            <h3 id="close-title" className="short-section-title" tabIndex={-1}>
                Close
            </h3>
            {venueCannot && (
                <div className="banner" data-tone="warn" role="status">
                    {snapshot.kind === 'pump_migrating'
                        ? errorByName('VenueMigrating')?.message
                        : errorByName('NoRoute')?.message}
                </div>
            )}
            {!venueCannot && mx.insufficientVault && (
                <div className="banner" data-tone="danger" role="status" data-code="InsufficientVault">
                    {errorByName('InsufficientVault')?.message} Add margin to close.
                </div>
            )}
            <Field label="Max slippage on the buyback" error={!slipOk ? 'Use 0.1–50%.' : null}>
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
                            data-testid={T.closeSlippageInput}
                            data-value={slipOk ? String(slip) : undefined}
                            onChange={(e) => setSlipText(e.target.value.replace(',', '.'))}
                        />
                        <span className="input-suffix">%</span>
                    </div>
                )}
            </Field>
            {/* A value's sub-line is a second dd spanning the row (a dt may have several dd; the value dd
                keeps its testid and data-value); the decisive figures carry data-emph (design pass P4). */}
            <dl className="kv">
                <dt>Buyback cost (SOL)</dt>
                <dd data-testid={T.closePreview('buyback')} data-value={preview?.buyback.toString()}>
                    {preview ? <Sol lamports={preview.buyback} /> : '—'}
                </dd>
                {preview && (
                    <dd className="short-preview-sub">
                        buys back <Tokens raw={pos.q} decimals={snapshot.decimals} symbol={symbol} /> · at most{' '}
                        <Sol lamports={preview.maxBuybackCost} unit={false} /> SOL
                    </dd>
                )}
                <dt>Rent</dt>
                <dd data-testid={T.closePreview('rent')} data-value={(preview?.rent ?? mx.rentDue).toString()}>
                    <Sol lamports={preview?.rent ?? mx.rentDue} frac={6} />
                </dd>
                <dt>You receive</dt>
                <dd data-testid={T.closePreview('payout')} data-value={preview?.payout.toString()} data-emph="">
                    {preview ? <Sol lamports={preview.payout} /> : '—'}
                </dd>
                <dd className="short-preview-sub">plus the position account’s rent back</dd>
                <dt>P&amp;L</dt>
                <dd data-testid={T.closePreview('pnl')} data-value={preview?.pnl.toString()} data-emph="">
                    {preview ? <Pnl lamports={preview.pnl} /> : '—'}
                </dd>
            </dl>
            <FormFoot reason={reason} reasonTestId={T.closeReason}>
                {(trade.buildError ?? submitError) && (
                    <div className="banner" data-tone="danger" role="alert">
                        {trade.buildError ?? submitError}
                    </div>
                )}
                <Button
                    variant="danger"
                    block
                    disabled={!canSubmit}
                    busy={trade.building}
                    onClick={() => void submit()}
                    data-testid={T.closeSubmit}
                >
                    {!connected ? 'Connect a wallet to close' : !mine ? 'Not your position' : 'Close position'}
                </Button>
            </FormFoot>
            {trade.fallback && (
                <p className="short-note" role="status">
                    The venue’s extra accounts changed; retried with the {trade.fallback} set.
                </p>
            )}
            <TxReviewModal flow={trade.flow} />
        </section>
    );
}

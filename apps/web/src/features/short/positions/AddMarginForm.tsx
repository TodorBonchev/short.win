/**
 * Add margin (FRONTEND.md §3.8, §5.10; ARCH §3.5): SOL into the position's vault, which only raises
 * its CR — allowed on every open position (paused, flagged, liquidatable, armed). The preview is
 * the CR after (`add-margin-preview-cr`) on the live risk quote; the amount is bounded by the
 * wallet's SOL less a 0.01 SOL fee reserve.
 */
import type { ShortPosition } from '@shortwin/client';
import type { Address } from '@solana/kit';
import { useState } from 'react';
import { Button } from '../../../components/Button.tsx';
import { AmountInput } from '../../../components/Field.tsx';
import { T } from '../../../config/testids.ts';
import { parseAmount } from '../../../lib/amount.ts';
import type { MathModule } from '../../../math/load.ts';
import { TxReviewModal } from '../../../tx/TxReviewModal.tsx';
import { useTxFlow } from '../../../tx/useTxFlow.ts';
import { useWallet } from '../../../wallet/useWallet.ts';
import { useSolBalance } from '../hooks.ts';
import { planAddMargin } from '../plans.ts';
import { FEE_RESERVE_LAMPORTS, positionMetrics, previewAddMargin } from '../previews.ts';
import type { ChainSnapshot } from '../snapshot.ts';
import { CrValue, plainAmount } from '../ui.tsx';

export function AddMarginForm({
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
    const flow = useTxFlow();
    const balance = useSolBalance(connected?.address ?? null).data ?? null;
    const [text, setText] = useState('');
    const [buildError, setBuildError] = useState<string | null>(null);
    const parsed = parseAmount(text, 9);
    const amount = parsed.ok ? parsed.raw : null;
    const max = balance !== null && balance > FEE_RESERVE_LAMPORTS ? balance - FEE_RESERVE_LAMPORTS : 0n;
    const tooMuch = amount !== null && balance !== null && amount > max;
    const before = positionMetrics(math, snapshot, pos);
    const crAfter = amount !== null ? previewAddMargin(math, snapshot, pos, amount) : null;
    const mine = connected?.address === pos.borrower;
    const stage = flow.state.stage;
    const canSubmit =
        !!connected?.signer &&
        mine &&
        amount !== null &&
        !tooMuch &&
        !stale &&
        (stage === 'idle' || stage === 'confirmed');

    const submit = async () => {
        if (!connected?.signer || amount === null) return;
        setBuildError(null);
        try {
            flow.start(
                await planAddMargin({
                    signer: connected.signer,
                    market: snapshot.marketAddress,
                    mint: snapshot.mint,
                    position,
                    amount,
                    decimals: snapshot.decimals,
                    symbol,
                }),
            );
        } catch (e) {
            setBuildError(e instanceof Error ? e.message : String(e));
        }
    };

    return (
        <section className="stack" id="add-margin" aria-labelledby="add-margin-title">
            <h3 id="add-margin-title" className="short-section-title">
                Add margin
            </h3>
            <AmountInput
                label="Amount (SOL)"
                value={text}
                onChange={(t) => setText(t)}
                decimals={9}
                unit="SOL"
                max={balance !== null ? max : undefined}
                formatMax={(raw) => plainAmount(raw, 9)}
                error={tooMuch ? 'Above your SOL balance less 0.01 SOL for fees.' : null}
                testId={T.addMarginInput}
                hint="Margin only raises your CR. It stays in the position until it closes."
            />
            <dl className="kv">
                <dt>CR now</dt>
                <dd>
                    <CrValue bps={before.crBps} liqBps={pos.crLiqBps} />
                </dd>
                <dt>CR after</dt>
                <dd data-testid={T.addMarginPreviewCr} data-value={crAfter?.toString()}>
                    <CrValue bps={crAfter} liqBps={pos.crLiqBps} />
                </dd>
            </dl>
            <Button
                variant="primary"
                block
                disabled={!canSubmit}
                onClick={() => void submit()}
                data-testid={T.addMarginSubmit}
            >
                {!connected ? 'Connect a wallet' : !mine ? 'Not your position' : 'Add margin'}
            </Button>
            {buildError && (
                <div className="banner" data-tone="danger" role="alert">
                    {buildError}
                </div>
            )}
            <TxReviewModal flow={flow} />
        </section>
    );
}

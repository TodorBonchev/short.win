/**
 * The review / signing modal every flow uses (FRONTEND.md §5.3): who signs (wallet and account),
 * the plan's instructions, simulated deltas (before → after, change), compute units, fee and size,
 * the plan's disclosures with their acknowledgement boxes, `tx.simulation`, the mapped error with
 * what Retry will do (`retryMode`), and after landing the signature with an explorer link.
 */
import { useMemo } from 'react';
import { Button } from '../components/Button.tsx';
import { Disclosure } from '../components/Disclosure.tsx';
import { Select } from '../components/Field.tsx';
import { Modal } from '../components/Modal.tsx';
import { explorerTx } from '../config/env.ts';
import { T } from '../config/testids.ts';
import { formatFixed, MINUS, shortAddress } from '../lib/format.ts';
import { change } from './deltas.ts';
import { describeInstruction } from './describe.ts';
import type { FeeSpeed } from './priorityFee.ts';
import type { Delta } from './types.ts';
import { canSign, type RetryMode, retryMode, type SigningWallet, type TxFlow } from './useTxFlow.ts';

const RETRY_TEXT: Record<Exclude<RetryMode, 'none'>, string> = {
    resimulate: 'Retry',
    rebuild: 'Retry with a new quote',
    close: 'Close and review again',
};

/** "Signing with <icon> <wallet> · <account>" (M9-SEC-8), and the sending-only caveat (§5.4). */
function SigningWith({ wallet }: { wallet: SigningWallet }) {
    return (
        <div className="stack" style={{ gap: 4 }}>
            <div
                className="row muted"
                data-testid={T.txWallet}
                data-value={wallet.address}
                data-sends-itself={String(wallet.sendsItself)}
            >
                <span>Signing with</span>
                {wallet.icon && <img src={wallet.icon} alt="" width={14} height={14} />}
                <bdi>{wallet.name}</bdi>
                <span className="num" title={wallet.address}>
                    · {shortAddress(wallet.address)}
                </span>
            </div>
            {wallet.sendsItself && (
                <div className="banner" data-tone="warn" role="note">
                    This wallet sends the transaction itself, so it cannot be checked before it is sent. The app
                    compares what lands with this review afterwards.
                </div>
            )}
        </div>
    );
}

const STAGE_TEXT = {
    idle: '',
    simulating: 'Simulating…',
    review: 'Review',
    signing: 'Waiting for your wallet…',
    confirming: 'Confirming…',
    confirmed: 'Confirmed',
    failed: 'Failed',
} as const;

function fmt(v: bigint | null, scale: number): string {
    return v === null ? '—' : formatFixed(v, scale, Math.min(scale, 6));
}

function DeltaRow({ d }: { d: Delta }) {
    const c = change(d);
    const sign = c > 0n ? 'up' : c < 0n ? 'down' : 'flat';
    const text =
        c === 0n ? '0' : `${c > 0n ? '+' : MINUS}${formatFixed(c < 0n ? -c : c, d.scale, Math.min(d.scale, 6))}`;
    return (
        <tr data-testid={T.txDelta(d.key)} data-value={c.toString()}>
            <td>{d.label}</td>
            <td data-align="right" className="num muted">
                {fmt(d.before, d.scale)}
            </td>
            <td data-align="right" className="num">
                {fmt(d.after, d.scale)}
            </td>
            <td data-align="right" className="num pnl" data-sign={sign}>
                {text} <span className="unit">{d.unit}</span>
            </td>
        </tr>
    );
}

export function TxReviewModal({ flow }: { flow: TxFlow }) {
    const { state } = flow;
    const { plan, report, stage, error } = state;
    const open = stage !== 'idle';
    const busy = stage === 'simulating' || stage === 'signing' || stage === 'confirming';
    const steps = useMemo(
        () => (plan?.instructions ?? []).map((ix, i) => ({ id: String(i), ...describeInstruction(ix) })),
        [plan],
    );
    const retry = retryMode(state);
    const showRetry = (stage === 'review' || stage === 'failed') && retry !== 'none';
    const retryText = stage === 'review' ? 'Refresh' : retry === 'none' ? '' : RETRY_TEXT[retry];
    const stageText = stage === 'simulating' && state.rebuilding ? 'Rebuilding…' : STAGE_TEXT[stage];
    return (
        <Modal
            open={open}
            onClose={flow.close}
            dismissible={!(stage === 'signing' || stage === 'confirming')}
            testId={T.txModal}
            title={plan?.title ?? 'Transaction'}
            footer={
                stage === 'confirmed' ? (
                    <Button variant="primary" onClick={flow.close}>
                        Done
                    </Button>
                ) : (
                    <>
                        <Button
                            variant="ghost"
                            onClick={flow.close}
                            disabled={stage === 'signing' || stage === 'confirming'}
                            data-testid={T.txCancel}
                        >
                            {stage === 'failed' && retry === 'none' ? 'Close' : 'Cancel'}
                        </Button>
                        {showRetry && (
                            <Button onClick={flow.refresh} data-testid={T.txRetry} data-mode={retry}>
                                {retryText}
                            </Button>
                        )}
                        <Button
                            variant="primary"
                            onClick={flow.sign}
                            disabled={!canSign(state)}
                            busy={busy}
                            data-testid={T.txSign}
                        >
                            Sign in wallet
                        </Button>
                    </>
                )
            }
        >
            <div className="row">
                <span data-testid={T.txStage} data-stage={stage} aria-live="polite">
                    {stageText}
                </span>
                {report && (
                    <span
                        className="badge"
                        data-tone={report.ok ? 'ok' : 'danger'}
                        data-testid={T.txSimStatus}
                        data-ok={String(report.ok)}
                    >
                        Simulation {report.ok ? 'OK' : 'failed'}
                    </span>
                )}
            </div>
            {flow.wallet && <SigningWith wallet={flow.wallet} />}
            {state.refreshed && stage === 'review' && (
                <div className="banner" data-tone="info" role="status">
                    The quote was older than 20 seconds and has been refreshed. Review the new numbers.
                </div>
            )}
            {state.rebuilt && (stage === 'review' || stage === 'simulating') && (
                <div className="banner" data-tone="info" role="status" data-testid={T.txRebuilt}>
                    Rebuilt from the chain as it is now: a new quote, new limits and any step it now needs. Review the
                    new numbers.
                </div>
            )}
            {steps.length > 0 && (
                <ol className="tx-steps" aria-label="Instructions in this transaction">
                    {steps.map((d) => (
                        <li
                            key={d.id}
                            data-testid={T.txInstruction(d.id)}
                            data-program={d.program}
                            data-name={d.name}
                            data-allowed={String(d.allowed)}
                        >
                            <span className="muted">{d.program}</span> {d.name}
                            {!d.allowed && (
                                <span className="badge" data-tone="danger">
                                    not allowed
                                </span>
                            )}
                        </li>
                    ))}
                </ol>
            )}
            {report && report.deltas.length > 0 && (
                <table className="tbl tx-deltas">
                    <caption className="visually-hidden">Simulated changes</caption>
                    <thead>
                        <tr>
                            <th scope="col">Account</th>
                            <th scope="col" data-align="right">
                                Before
                            </th>
                            <th scope="col" data-align="right">
                                After
                            </th>
                            <th scope="col" data-align="right">
                                Change
                            </th>
                        </tr>
                    </thead>
                    <tbody>
                        {report.deltas.map((d) => (
                            <DeltaRow key={d.key} d={d} />
                        ))}
                    </tbody>
                </table>
            )}
            {report?.ok && (
                <dl className="kv">
                    <dt>Compute units</dt>
                    <dd className="num" data-testid={T.txCu} data-value={String(report.computeUnitLimit)}>
                        {report.unitsConsumed?.toString() ?? '—'} used · limit {report.computeUnitLimit}
                    </dd>
                    <dt>Network fee</dt>
                    <dd className="num" data-testid={T.txFee} data-value={report.feeLamports.toString()}>
                        {formatFixed(report.feeLamports, 9, 6)} SOL
                    </dd>
                    <dt>Priority</dt>
                    <dd>
                        <Select<FeeSpeed>
                            label="Priority fee"
                            value={state.speed}
                            onChange={(s) => flow.setSpeed(s)}
                            options={[
                                { value: 'normal', label: 'Normal' },
                                { value: 'fast', label: 'Fast' },
                            ]}
                        />
                    </dd>
                </dl>
            )}
            {plan?.disclosures.map((id) => (
                <Disclosure
                    key={id}
                    id={id}
                    vars={plan.disclosureVars?.[id]}
                    acknowledged={state.acks.has(id)}
                    onAcknowledge={(v) => flow.setAck(id, v)}
                />
            ))}
            {plan && <Disclosure id="tx.simulation" collapsible />}
            {error && (
                <div
                    className="banner"
                    data-tone="danger"
                    role="alert"
                    data-testid={T.txError}
                    data-code={error.code !== undefined ? String(error.code) : undefined}
                    data-kind={error.kind}
                >
                    <div>
                        <strong>{error.name ?? 'Error'}</strong> {error.message}
                    </div>
                </div>
            )}
            {state.finalFeeLamports != null && (
                <div className="row" data-testid={T.txFinalFee} data-value={state.finalFeeLamports.toString()}>
                    <span className="muted">Network fee as your wallet signed it</span>
                    <span className="num">{formatFixed(state.finalFeeLamports, 9, 6)} SOL</span>
                </div>
            )}
            {state.sentByWallet && stage === 'confirmed' && (
                <div className="banner" data-tone="ok" role="status">
                    Your wallet sent this transaction itself; what landed matches this review.
                </div>
            )}
            {state.signature && (
                <div className="row">
                    <span className="muted">Signature</span>
                    <span className="num" data-testid={T.txSignature} data-value={state.signature}>
                        {shortAddress(state.signature, 8, 8)}
                    </span>
                    <a
                        href={explorerTx(state.signature)}
                        target="_blank"
                        rel="noopener noreferrer"
                        data-testid={T.txExplorer}
                    >
                        Explorer
                    </a>
                </div>
            )}
        </Modal>
    );
}

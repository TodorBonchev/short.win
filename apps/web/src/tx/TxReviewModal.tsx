/**
 * The review / signing modal every flow uses (FRONTEND.md §5.3): who signs (wallet and account),
 * the plan's instructions, simulated deltas (before → after, change), compute units, fee and size,
 * the plan's disclosures with their acknowledgement boxes, `tx.simulation`, the mapped error with
 * what Retry will do (`retryMode`), and after landing the signature with an explorer link.
 *
 * M10-RESP (§5.3, §7.6): the error sits right under the stage; the simulation badge goes once the
 * wallet has it; a wallet rejection reads "Cancelled in your wallet" with one way on (Try again);
 * a timestamp delta is a UTC date; while an acknowledgement blocks Sign a hint says so and takes
 * focus to it; a simulation the plan holds (`TxPlan.holdSign`, a claim that pays 0) says why
 * beside the disabled Sign; once confirmed, the flow's next step (or the explorer). On a phone the dialog fills
 * the screen and Sign — last in the DOM — has the full-width bottom row; in a narrow dialog each
 * delta is two lines.
 *
 * The Mobile Wallet Adapter (O-6, §5.4 (6)): its notices beside an enabled Sign (`tx-sign-notice`:
 * the local-network permission, or a launch to retry), and its way out of a call that never
 * settles: Cancel, ✕ and Escape work while signing once `signCancellable` (R-S1).
 */
/* biome-ignore-all lint/a11y/noRedundantRoles: CSS display change (narrow dialog); roles restore semantics */
import { useRouter } from '@tanstack/react-router';
import { createContext, Fragment, type ReactNode, useContext, useMemo } from 'react';
import { Button } from '../components/Button.tsx';
import { Disclosure } from '../components/Disclosure.tsx';
import { Modal } from '../components/Modal.tsx';
import { Reveal } from '../components/Reveal.tsx';
import { walletCopy } from '../config/copyWallet.ts';
import { explorerAddress, explorerTx } from '../config/env.ts';
import { T } from '../config/testids.ts';
import { formatFixed, MINUS, shortAddress } from '../lib/format.ts';
import { formatUtc } from '../lib/time.ts';
import { change } from './deltas.ts';
import { describeInstruction } from './describe.ts';
import type { FeeSpeed } from './priorityFee.ts';
import type { Delta } from './types.ts';
import {
    canSign,
    type RetryMode,
    requiredAcks,
    retryMode,
    type SigningWallet,
    signHold,
    type TxFlow,
} from './useTxFlow.ts';

/** The priority fee's two speeds, as the review offers them (both visible: a chip pair, not a menu). */
const SPEEDS: readonly (readonly [FeeSpeed, string])[] = [
    ['normal', 'Normal'],
    ['fast', 'Fast'],
];

const RETRY_TEXT: Record<Exclude<RetryMode, 'none'>, string> = {
    resimulate: 'Retry',
    rebuild: 'Retry with a new quote',
    close: 'Close and review again',
};

/** "Signing with <icon> <wallet> · <account>" (M9-SEC-8), and the sending-only caveat (§5.4). */
function SigningWith({ wallet }: { wallet: SigningWallet }) {
    return (
        <div className="stack-tight">
            <div
                className="row muted"
                data-testid={T.txWallet}
                data-value={wallet.address}
                data-sends-itself={String(wallet.sendsItself)}
            >
                <span>Signing with</span>
                {wallet.icon && <img src={wallet.icon} alt="" width={14} height={14} />}
                <bdi>{wallet.name}</bdi>
                <span aria-hidden="true">·</span>
                <Reveal value={wallet.address} kind="wallet" label="Account" explorer={explorerAddress(wallet.address)}>
                    <span className="num">{shortAddress(wallet.address)}</span>
                </Reveal>
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

/**
 * What closing a confirmed review does besides closing it: the page that hosts the flow passes a
 * callback while the flow sits in a sheet (a phone's or tablet's trade or Manage sheet), so Done —
 * or ✕, or Escape — also closes the sheet and the user lands on the page, its toast whole
 * (FRONTEND.md §7.6). Not after a failure or a cancel: the sheet stays for another try.
 */
export const TxDoneContext = createContext<(() => void) | null>(null);

/** A shortened id ("8oBK…6cTC") never splits across lines: each one is wrapped in a nowrap span. */
const SHORT_ID = /(\S+…\S+)/;
export function keepIds(text: string): ReactNode {
    const parts = text.split(SHORT_ID);
    if (parts.length === 1) return text;
    return parts.map((part, i) =>
        i % 2 === 1 ? (
            // biome-ignore lint/suspicious/noArrayIndexKey: the parts of one string, in their order
            <span key={i} className="nowrap">
                {part}
            </span>
        ) : (
            // biome-ignore lint/suspicious/noArrayIndexKey: the parts of one string, in their order
            <Fragment key={i}>{part}</Fragment>
        ),
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
    const timestamp = d.kind === 'timestamp';
    // A date may break between its day and its time (never inside either), so it does not widen
    // the AFTER column and squeeze the account label.
    const date = (v: bigint) => {
        const [day, ...time] = formatUtc(v).split(' ');
        return (
            <>
                <span className="nowrap">{day}</span> <span className="nowrap">{time.join(' ')}</span>
            </>
        );
    };
    const show = (v: bigint | null) => (v === null ? '—' : timestamp ? date(v) : fmt(v, d.scale));
    const text = timestamp
        ? ''
        : c === 0n
          ? '0'
          : `${c > 0n ? '+' : MINUS}${formatFixed(c < 0n ? -c : c, d.scale, Math.min(d.scale, 6))}`;
    const unit = timestamp ? null : <span className="unit">{d.unit}</span>;
    // The CHANGE cell stays (and its testid's row keeps `data-value`); a narrow dialog shows before
    // → after and the change inside the values cell instead (`.delta-inline`, aria-hidden copies).
    return (
        <tr role="row" data-testid={T.txDelta(d.key)} data-value={c.toString()} data-kind={d.kind}>
            <td role="cell">{keepIds(d.label)}</td>
            <td role="cell" data-align="right" className="num muted delta-before">
                {show(d.before)}
            </td>
            <td role="cell" data-align="right" className="num delta-vals">
                <span className="delta-inline muted" aria-hidden="true">
                    {show(d.before)} →{' '}
                </span>
                <span>{show(d.after)}</span>
                {!timestamp && (
                    <span className="delta-inline num pnl" data-sign={sign} aria-hidden="true">
                        {text} {unit}
                    </span>
                )}
            </td>
            <td
                role="cell"
                data-align="right"
                className="num pnl delta-change"
                data-sign={timestamp ? undefined : sign}
            >
                {text} {unit}
            </td>
        </tr>
    );
}

export function TxReviewModal({ flow }: { flow: TxFlow }) {
    const { state } = flow;
    const router = useRouter({ warn: false }) as ReturnType<typeof useRouter> | undefined;
    const onDone = useContext(TxDoneContext);
    const { plan, report, stage, error } = state;
    const rejected = stage === 'failed' && error?.kind === 'wallet-rejected';
    // The simulation badge describes the review; once the wallet has the transaction it is noise.
    const showSim =
        !!report &&
        (stage === 'simulating' || stage === 'review' || (stage === 'failed' && !error?.kind.startsWith('wallet')));
    // A simulation the plan itself refuses to sign (`holdSign`: a claim that pays 0) says why
    // beside the disabled Sign; it outranks an unticked acknowledgement (ticking would not help).
    const hold = signHold(state);
    const ackBlocks =
        hold === null && stage === 'review' && !!report?.ok && requiredAcks(plan).some((id) => !state.acks.has(id));
    const open = stage !== 'idle';
    // Never closable while confirming, nor while the wallet is asked to sign — except the Mobile
    // Wallet Adapter's way out of a call that never settles (R-S1: Cancel, ✕, Escape after 5 s).
    const locked = stage === 'confirming' || (stage === 'signing' && !state.signCancellable);
    // Every way out of a confirmed review ends in the same place (Done, ✕, Escape).
    const close =
        stage === 'confirmed'
            ? () => {
                  flow.close();
                  onDone?.();
              }
            : flow.close;
    const busy = stage === 'simulating' || stage === 'signing' || stage === 'confirming';
    const steps = useMemo(
        () => (plan?.instructions ?? []).map((ix, i) => ({ id: String(i), ...describeInstruction(ix) })),
        [plan],
    );
    const retry = retryMode(state);
    const showRetry = (stage === 'review' || stage === 'failed') && retry !== 'none';
    const retryText =
        stage === 'review' ? 'Refresh' : rejected ? 'Try again' : retry === 'none' ? '' : RETRY_TEXT[retry];
    const focusFirstAck = () => {
        const dialog = document.querySelector(`dialog[data-testid="${T.txModal}"][open]`);
        const box = dialog?.querySelector<HTMLInputElement>(
            'input[type="checkbox"][data-testid^="disclosure-ack-"]:not(:checked)',
        );
        box?.scrollIntoView?.({ block: 'center' });
        box?.focus();
    };
    // The user's own cancel is not a failure (D4): no red-flow word above "Cancelled in your wallet".
    const stageText = rejected
        ? 'Cancelled'
        : stage === 'simulating' && state.rebuilding
          ? 'Rebuilding…'
          : STAGE_TEXT[stage];
    return (
        <Modal
            open={open}
            onClose={close}
            dismissible={!locked}
            testId={T.txModal}
            title={keepIds(plan?.title ?? 'Transaction')}
            footer={
                stage === 'confirmed' ? (
                    <Button variant="primary" onClick={close}>
                        Done
                    </Button>
                ) : (
                    <>
                        {hold !== null && (
                            <p className="form-reason tx-sign-hint" role="status" data-testid={T.txSignHold}>
                                {hold}
                            </p>
                        )}
                        {ackBlocks && (
                            <button
                                type="button"
                                className="form-reason tx-sign-hint"
                                data-testid={T.txSignHint}
                                onClick={focusFirstAck}
                            >
                                Tick the acknowledgement above to sign
                            </button>
                        )}
                        {state.signNotice && (
                            <p
                                className="form-reason tx-sign-hint"
                                role="status"
                                data-testid={T.txSignNotice}
                                data-reason={state.signNotice}
                            >
                                {walletCopy(
                                    state.signNotice === 'local-network'
                                        ? 'wallet.mwa.localNetwork'
                                        : 'wallet.mwa.retry',
                                )}
                            </p>
                        )}
                        <Button
                            variant="ghost"
                            onClick={flow.close}
                            disabled={locked}
                            data-testid={T.txCancel}
                            data-cancellable={String(!locked)}
                        >
                            {stage === 'failed' && retry === 'none' ? 'Close' : 'Cancel'}
                        </Button>
                        {showRetry && (
                            <Button
                                variant={rejected ? 'primary' : 'default'}
                                onClick={flow.refresh}
                                data-testid={T.txRetry}
                                data-mode={retry}
                            >
                                {retryText}
                            </Button>
                        )}
                        {!rejected && (
                            <Button
                                variant="primary"
                                onClick={flow.sign}
                                disabled={!canSign(state)}
                                busy={busy}
                                data-testid={T.txSign}
                            >
                                Sign in wallet
                            </Button>
                        )}
                    </>
                )
            }
        >
            <div className="row">
                <span data-testid={T.txStage} data-stage={stage} aria-live="polite">
                    {stageText}
                </span>
                {report && showSim && (
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
            {error && (
                <div
                    className="banner"
                    data-tone={rejected ? 'info' : 'danger'}
                    role="alert"
                    data-testid={T.txError}
                    data-code={error.code !== undefined ? String(error.code) : undefined}
                    data-kind={error.kind}
                >
                    {rejected ? (
                        <div>
                            <strong>Cancelled in your wallet.</strong> Nothing was sent.
                        </div>
                    ) : (
                        <div>
                            <strong>{error.name ?? 'Error'}</strong> {error.message}
                        </div>
                    )}
                </div>
            )}
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
                <table className="tbl tx-deltas" role="table">
                    <caption className="visually-hidden">Simulated changes</caption>
                    <thead role="rowgroup">
                        <tr role="row">
                            <th role="columnheader" scope="col">
                                Account
                            </th>
                            <th role="columnheader" scope="col" data-align="right">
                                Before
                            </th>
                            <th role="columnheader" scope="col" data-align="right">
                                After
                            </th>
                            <th role="columnheader" scope="col" data-align="right">
                                Change
                            </th>
                        </tr>
                    </thead>
                    <tbody role="rowgroup">
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
                        {/* The one choice the review offers, whole: the pressed chip is the selected state
                            (`.chip[aria-pressed]`); `hit` gives each a 44 px touch area under a finger. A
                            fieldset with a hidden legend names the pair, as the withdraw shares do. */}
                        <fieldset className="chip-group tx-priority">
                            <legend className="visually-hidden">Priority fee</legend>
                            {SPEEDS.map(([speed, label]) => (
                                <button
                                    key={speed}
                                    type="button"
                                    className="chip hit"
                                    aria-pressed={state.speed === speed}
                                    onClick={() => flow.setSpeed(speed)}
                                >
                                    {label}
                                </button>
                            ))}
                        </fieldset>
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
            {state.finalFeeLamports != null && (
                <div className="row" data-testid={T.txFinalFee} data-value={state.finalFeeLamports.toString()}>
                    <span className="muted">Network fee as your wallet signed it</span>
                    <span className="num">{formatFixed(state.finalFeeLamports, 9, 6)} SOL</span>
                </div>
            )}
            {/* The flow's next step, when it has one; the explorer link is the signature row's (one
                explorer link, not two). */}
            {stage === 'confirmed' && (
                <div className="banner" data-tone="ok" role="status">
                    <span className="grow">Confirmed on chain.</span>
                    {plan?.next && (
                        <a
                            href={plan.next.href}
                            className="target-link"
                            data-testid={T.txConfirmedLink}
                            onClick={(e) => {
                                if (!router || !plan.next) return;
                                e.preventDefault();
                                flow.close();
                                void router.navigate({ href: plan.next.href });
                            }}
                        >
                            {plan.next.text}
                        </a>
                    )}
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
                        className="target-link"
                        data-testid={T.txExplorer}
                    >
                        Open in explorer
                    </a>
                </div>
            )}
        </Modal>
    );
}

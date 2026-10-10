/**
 * Lend → Withdraw (FRONTEND.md §3.6): shares (or a share of the holding; Max = the pro-rata
 * allowance), the preview (tokens, SOL, allowance, the lock now, the lock the withdrawal leaves
 * and until when (`previewWithdrawal`), when a new lock would end, the vest the withdrawal gives
 * up), the destination account (the ATA, created idempotently when missing; a memo-required
 * destination gets `memo_program`), `lender.exitForfeitsVest` with its checkbox, and the review
 * with `min_tokens` (the tolerance below the quote) and `min_sol` (the quoted SOL part, vest
 * included: `quote.ts`).
 */
import type { Address } from '@solana/kit';
import { useEffect, useId, useState } from 'react';
import { Button } from '../../../components/Button.tsx';
import { Disclosure } from '../../../components/Disclosure.tsx';
import { AmountInput } from '../../../components/Field.tsx';
import { FormFoot } from '../../../components/FormFoot.tsx';
import { Sol, Tokens } from '../../../components/Num.tsx';
import { Kv } from '../../../components/States.tsx';
import { T } from '../../../config/testids.ts';
import { parseAmount } from '../../../lib/amount.ts';
import { formatSol, formatTokens } from '../../../lib/format.ts';
import { formatUtc } from '../../../lib/time.ts';
import { ConnectCta } from '../../../wallet/ConnectCta.tsx';
import { withdrawPlanFrom } from '../build.ts';
import {
    DEFAULT_TOLERANCE,
    type LendCtx,
    lendEnvOf,
    lendStateOf,
    lockGraceOf,
    nowFor,
    parseTolerance,
    toInputText,
} from '../ctx.ts';
import { previewWithdrawLimits, previewWithdrawView, standingOf } from '../previews.ts';
import {
    type AccountOption,
    AccountSelect,
    AckTarget,
    accountLabel,
    type Reason,
    RefusalLine,
    ToleranceInput,
} from '../ui.tsx';

/** The quick shares of the holding (chips); Max — what may be withdrawn now — sits in the field. */
const PERCENTS = ['25', '50', '75'] as const;

export function WithdrawForm({ ctx, onCoverDeficit }: { ctx: LendCtx; onCoverDeficit: () => void }) {
    const { chain, math, decimals, symbol, signer, flow } = ctx;
    const [sharesText, setSharesText] = useState('');
    const [destAddr, setDestAddr] = useState<string | null>(null);
    const [tolText, setTolText] = useState(DEFAULT_TOLERANCE);
    const [ack, setAck] = useState(false);
    const [building, setBuilding] = useState(false);
    const [buildError, setBuildError] = useState<string | null>(null);
    const ackId = useId();

    const stage = flow.state.stage;
    const planKind = flow.state.plan?.kind;
    useEffect(() => {
        if (stage === 'confirmed' && planKind === 'withdraw') {
            setAck(false);
            setSharesText('');
        }
    }, [stage, planKind]);

    const state = lendStateOf(chain);
    const now = nowFor(ctx, 'market');
    const grace = lockGraceOf(chain);
    const limits = previewWithdrawLimits(math, state, now, grace);
    const parsed = parseAmount(sharesText, decimals);
    const shares = parsed.ok ? parsed.raw : null;
    const tolBps = parseTolerance(tolText);
    const view =
        shares !== null && tolBps !== null ? previewWithdrawView(math, state, now, grace, shares, tolBps) : null;
    const holding = limits.ok ? limits.value.shares : 0n;
    const overHolding = shares !== null && shares > holding;

    // Destinations: the owner's accounts of the mint (ATA first); a missing ATA is created idempotently.
    const accounts = chain.tokenAccounts;
    const ataExists = accounts.some((a) => a.address === chain.ata);
    const options: AccountOption[] = [
        ...(chain.ata && !ataExists ? [{ address: chain.ata, label: 'Main (ATA) · created with the withdrawal' }] : []),
        ...accounts.map((a) => ({ address: a.address, label: accountLabel(a, chain.ata, decimals, symbol) })),
    ];
    const dest = options.find((o) => o.address === destAddr) ?? options[0] ?? null;
    const destInfo = dest ? (accounts.find((a) => a.address === dest.address) ?? null) : null;
    const memo = destInfo?.memoRequired ?? false;
    const forfeit = view?.ok ? view.value.vestingForfeit : 0n;

    // Why the review cannot open yet: the same predicates as `ready`, first unmet first (§7.6).
    const blocker = ((): Reason | null => {
        if (!signer) return { text: 'Connect a wallet first' };
        if (!dest) return { text: 'No account to receive the tokens' };
        if (!chain.recordsComplete) return { text: 'Sync your position first (above)' };
        if (ctx.stale) return { text: 'Waiting for a fresh chain read' };
        if (shares === null) return { text: 'Enter the shares to withdraw' };
        if (overHolding) return { text: 'More shares than you hold' };
        if (tolBps === null) return { text: 'Set a tolerance from 0.1 to 50 %' };
        if (!view?.ok) return { text: 'The program would refuse this withdrawal (see above)' };
        if (forfeit > 0n && !ack) return { text: 'Tick the acknowledgement above to continue', target: ackId };
        return null;
    })();
    const ready = blocker === null && !building;

    const submit = async () => {
        if (!signer || !dest || shares === null || !view?.ok || tolBps === null) return;
        setBuilding(true);
        setBuildError(null);
        try {
            // The same builder the review's Retry runs on a fresh read (`build.ts`, M9-UX-1): the
            // destination's memo flag, `min_tokens` and the simulated `min_sol` come from that read.
            flow.start(
                await withdrawPlanFrom(lendEnvOf(ctx, signer), chain, ctx.nowS, {
                    shares,
                    destination: dest.address as Address,
                    toleranceBps: tolBps,
                }),
            );
        } catch (e) {
            setBuildError(e instanceof Error ? e.message : String(e));
        } finally {
            setBuilding(false);
        }
    };

    if (!signer) {
        return (
            <div className="stack lend-form">
                <div className="muted">Connect a wallet to see your shares in this market.</div>
                <ConnectCta surface="lend" />
            </div>
        );
    }
    if (!chain.lender || (limits.ok && limits.value.shares === 0n && chain.lender.shares === 0n)) {
        return <div className="muted lend-form">You have no shares in this market.</div>;
    }
    if (standingOf(chain.market, chain.lender).kind === 'parked') {
        return (
            <div className="muted lend-form">
                A full settlement wiped this pool out: your shares hold no tokens to withdraw. Claim the SOL it left you
                on the Claim tab.
            </div>
        );
    }
    const lim = limits.ok ? limits.value : null;
    const solShown = view?.ok ? view.value.solAtLeast : null;
    const percentText = (pct: (typeof PERCENTS)[number]) => toInputText((holding * BigInt(pct)) / 100n, decimals);

    return (
        <div className="stack lend-form">
            <AmountInput
                label="Shares"
                value={sharesText}
                onChange={(t) => setSharesText(t)}
                decimals={decimals}
                unit="shares"
                max={lim?.maxShares}
                formatMax={(raw) => toInputText(raw, decimals)}
                error={overHolding ? 'More shares than you hold.' : null}
                testId={T.withdrawSharesInput}
                maxTestId={T.withdrawMax}
                disabled={!signer}
                hint={
                    lim
                        ? `You hold ${formatTokens(lim.shares, decimals)} shares; Max = what you may withdraw now.`
                        : undefined
                }
            />
            {view?.ok && (
                <div className="lend-approx" data-testid={T.withdrawApprox}>
                    ≈ <Tokens raw={view.value.tokens} decimals={decimals} symbol={symbol} />
                </div>
            )}
            <fieldset className="chip-group lend-quick">
                <legend className="visually-hidden">Share of your holding</legend>
                {PERCENTS.map((pct) => (
                    <button
                        key={pct}
                        type="button"
                        className="chip hit"
                        aria-pressed={sharesText !== '' && sharesText === percentText(pct)}
                        disabled={!lim}
                        data-testid={T.withdrawPercent(pct)}
                        onClick={() => setSharesText(percentText(pct))}
                    >
                        {pct}%
                    </button>
                ))}
            </fieldset>
            <ToleranceInput
                label="Min tokens tolerance"
                value={tolText}
                onChange={setTolText}
                invalid={tolBps === null}
            />
            {view && !view.ok && (
                <>
                    <RefusalLine refusal={view.refusal} />
                    {view.refusal.name === 'NegativePending' && (
                        <Button size="sm" onClick={onCoverDeficit}>
                            Go to cover deficit
                        </Button>
                    )}
                </>
            )}
            <Kv
                rows={[
                    [
                        'You may withdraw now',
                        lim ? (
                            <Tokens
                                key="r191"
                                raw={lim.allowance}
                                decimals={decimals}
                                symbol={symbol}
                                testId={T.withdrawPreview('allowance')}
                            />
                        ) : (
                            '—'
                        ),
                    ],
                    [
                        'Tokens',
                        view?.ok ? (
                            <Tokens
                                key="r204"
                                raw={view.value.tokens}
                                decimals={decimals}
                                symbol={symbol}
                                testId={T.withdrawPreview('tokens')}
                            />
                        ) : (
                            '—'
                        ),
                    ],
                    [
                        view?.ok && view.value.lastLenderExit ? 'SOL (incl. the vest, at least)' : 'SOL (at least)',
                        solShown !== null ? (
                            <Sol key="r217" lamports={solShown} frac={6} testId={T.withdrawPreview('sol')} />
                        ) : (
                            '—'
                        ),
                    ],
                    [
                        'Locked shares now',
                        lim ? (
                            <Tokens key="r224" raw={lim.lock} decimals={decimals} testId={T.withdrawPreview('lock')} />
                        ) : (
                            '—'
                        ),
                    ],
                    ...(lim && lim.lock > 0n
                        ? ([['Current lock ends', <span key="cur">{formatUtc(lim.lockUntil)}</span>]] as const)
                        : []),
                    [
                        'Locked shares after',
                        view?.ok ? (
                            <span
                                key="after"
                                className="num"
                                data-testid={T.withdrawPreview('lock-after')}
                                data-value={view.value.lockAfter.toString()}
                                data-until={view.value.lockUntilAfter.toString()}
                            >
                                {view.value.lockAfter > 0n ? (
                                    <>
                                        {formatTokens(view.value.lockAfter, decimals)}{' '}
                                        <span className="lend-nowrap">
                                            until {formatUtc(view.value.lockUntilAfter)}
                                        </span>
                                    </>
                                ) : (
                                    'none'
                                )}
                            </span>
                        ) : (
                            '—'
                        ),
                    ],
                    [
                        'A new lock would end',
                        lim ? (
                            <span
                                key="r232"
                                className="num"
                                data-testid={T.withdrawPreview('lock-until')}
                                data-value={chain.market.borrowedQ > 0n ? lim.lockUntilIfLocked.toString() : '0'}
                            >
                                {chain.market.borrowedQ > 0n
                                    ? formatUtc(lim.lockUntilIfLocked)
                                    : 'no lock: nothing is borrowed'}
                            </span>
                        ) : (
                            '—'
                        ),
                    ],
                    ...(forfeit > 0n
                        ? ([
                              [
                                  'Vest you give up',
                                  <Sol
                                      key="forfeit"
                                      lamports={forfeit}
                                      frac={6}
                                      testId={T.withdrawPreview('vesting-forfeit')}
                                  />,
                              ],
                          ] as const)
                        : []),
                ]}
            />
            <div className="muted lend-note">
                While tokens are borrowed, the borrowed part of what you withdraw becomes a lock on your remaining
                shares until the positions it stands for have ended (plus the grace period).
            </div>
            {dest && (
                <AccountSelect
                    label="To account"
                    value={dest.address}
                    options={options}
                    onChange={setDestAddr}
                    testId={T.withdrawDestination}
                    data={{ 'data-memo': String(memo) }}
                />
            )}
            {memo && (
                <div className="lend-notice" data-testid={T.withdrawMemoNotice}>
                    <Disclosure id="token.memoRequired" />
                </div>
            )}
            {forfeit > 0n && view?.ok && (
                <AckTarget id={ackId}>
                    <Disclosure
                        id="lender.exitForfeitsVest"
                        vars={{ vesting: formatSol(view.value.vesting, 6) }}
                        acknowledged={ack}
                        onAcknowledge={setAck}
                    />
                </AckTarget>
            )}
            {buildError && (
                <div className="field-error" role="alert">
                    {buildError}
                </div>
            )}
            <FormFoot reason={blocker?.text ?? null} reasonTestId={T.lendWithdrawReason} reasonTarget={blocker?.target}>
                <Button
                    variant="primary"
                    block
                    busy={building}
                    disabled={!ready}
                    onClick={() => void submit()}
                    data-testid={T.withdrawSubmit}
                >
                    {building ? 'Quoting…' : 'Review withdrawal'}
                </Button>
            </FormFoot>
        </div>
    );
}

/**
 * Lend → Deposit (FRONTEND.md §3.6): source account (ATA first; CPI Guard detected per account),
 * the SPEC §5.3 disclosure inline above the amount with its required checkbox, the amount (Max =
 * the source's balance), the preview (shares minted, share of the pool, token value; WASM on the
 * chain state), the pool's unpaid rent — the stored receivable plus the rent the deposit's own
 * prologue credits first — and this deposit's part of it (`lender.unpaidRent`: collapsed, or expanded
 * and repeated in the review when material; ECON §9.4) and the review. A CPI-Guarded
 * source blocks the plain deposit and offers the opt-in bracket (FE-1); a live wipe record
 * prepends the post-wipe sweep bundle; a mint flagged unsafe blocks deposits.
 */
import { getLenderPositionSize } from '@shortwin/client';
import { useQuery } from '@tanstack/react-query';
import { useEffect, useId, useState } from 'react';
import { qk } from '../../../api/keys.ts';
import { Button } from '../../../components/Button.tsx';
import { Disclosure } from '../../../components/Disclosure.tsx';
import { AmountInput, Checkbox } from '../../../components/Field.tsx';
import { FormFoot } from '../../../components/FormFoot.tsx';
import { Bps, Sol, Tokens } from '../../../components/Num.tsx';
import { Kv } from '../../../components/States.tsx';
import { lenderRiskVars } from '../../../config/disclosureVars.ts';
import { T } from '../../../config/testids.ts';
import { parseAmount } from '../../../lib/amount.ts';
import { formatTokens } from '../../../lib/format.ts';
import { ConnectCta } from '../../../wallet/ConnectCta.tsx';
import { depositPlanFrom } from '../build.ts';
import {
    DEFAULT_TOLERANCE,
    disclosureVarsOf,
    type LendCtx,
    lendEnvOf,
    lendStateOf,
    nowFor,
    parseTolerance,
    toInputText,
    unpaidRentVarsOf,
} from '../ctx.ts';
import {
    hasFlag,
    MARKET_FLAG,
    pendingLenderCredit,
    previewDepositView,
    previewUnpaidRent,
    wipeSweepPending,
} from '../previews.ts';
import { AccountSelect, AckTarget, accountLabel, Blocked, type Reason, RefusalLine, ToleranceInput } from '../ui.tsx';
import { LenderRisks } from './LenderRisks.tsx';

export function DepositForm({ ctx }: { ctx: LendCtx }) {
    const { chain, math, decimals, symbol, signer, flow } = ctx;
    const market = chain.market;
    const accounts = chain.tokenAccounts;
    const [sourceAddr, setSourceAddr] = useState<string | null>(null);
    const source = accounts.find((a) => a.address === sourceAddr) ?? accounts[0] ?? null;
    const [amountText, setAmountText] = useState('');
    const [ack, setAck] = useState(false);
    const [bracketFor, setBracketFor] = useState<string | null>(null);
    const [tolText, setTolText] = useState(DEFAULT_TOLERANCE);
    const [building, setBuilding] = useState(false);
    const [buildError, setBuildError] = useState<string | null>(null);
    const ackId = useId();
    const bracketId = useId();

    // Acknowledgements are per transaction (FRONTEND.md §4.1 rule 3): a landed deposit clears them.
    const stage = flow.state.stage;
    const planKind = flow.state.plan?.kind;
    useEffect(() => {
        if (stage === 'confirmed' && planKind === 'deposit') {
            setAck(false);
            setAmountText('');
            setBracketFor(null);
        }
    }, [stage, planKind]);

    const rent = useQuery({
        queryKey: [...qk.chain, 'rent-exempt', getLenderPositionSize()],
        queryFn: () => ctx.rpc.getMinimumBalanceForRentExemption(BigInt(getLenderPositionSize())).send(),
        staleTime: Number.POSITIVE_INFINITY,
        enabled: !!signer && !chain.lender,
    });

    const parsed = parseAmount(amountText, decimals);
    const amount = parsed.ok ? parsed.raw : null;
    const tolBps = parseTolerance(tolText);
    const overBalance = amount !== null && source !== null && amount > source.amount;
    const sweep = wipeSweepPending(market);
    const unsafe = hasFlag(market, MARKET_FLAG.mintUnsafe);
    const guard = source?.cpiGuard ?? false;
    const bracket = guard && bracketFor === source?.address;
    const now = nowFor(ctx, 'market');
    const preview =
        amount !== null && tolBps !== null
            ? previewDepositView(math, lendStateOf(chain), now, amount, tolBps, sweep)
            : null;
    const vars = disclosureVarsOf(market, ctx.detail);
    // The receivable the deposit meets: the stored one plus what its prologue credits first.
    const poolUnpaid = market.receivable + pendingLenderCredit(math, market, now);
    const unpaid = preview?.ok ? previewUnpaidRent(math, market, now, preview.value.shares, amount ?? 0n) : null;

    // Why the review cannot open yet: the same predicates as `ready`, first unmet first (§7.6).
    const blocker = ((): Reason | null => {
        if (!signer) return { text: 'Connect a wallet first' };
        if (unsafe) return { text: 'Deposits are blocked on this market' };
        if (!source) return { text: `You hold no ${symbol} in this wallet` };
        if (!chain.recordsComplete) return { text: 'Sync your position first (above)' };
        if (ctx.stale) return { text: 'Waiting for a fresh chain read' };
        if (amount === null) return { text: 'Enter an amount' };
        if (overBalance) return { text: 'The amount is above the balance of this account' };
        if (tolBps === null) return { text: 'Set a tolerance from 0.1 to 50 %' };
        if (!preview?.ok) return { text: 'The program would refuse this deposit (see above)' };
        if (guard && !bracket) return { text: 'Turn CPI Guard off for this deposit (above)', target: bracketId };
        if (!ack) return { text: 'Tick the acknowledgement above to continue', target: ackId };
        return null;
    })();
    const ready = blocker === null && !building;

    const submit = async () => {
        if (!signer || !source || amount === null || !preview?.ok || tolBps === null) return;
        setBuilding(true);
        setBuildError(null);
        try {
            // The same builder the review's Retry runs on a fresh read (`build.ts`, M9-UX-1).
            const plan = await depositPlanFrom(lendEnvOf(ctx, signer), chain, ctx.nowS, {
                source: source.address,
                amount,
                toleranceBps: tolBps,
                bracket,
            });
            flow.start(plan);
        } catch (e) {
            setBuildError(e instanceof Error ? e.message : String(e));
        } finally {
            setBuilding(false);
        }
    };

    return (
        <div className="stack lend-form">
            {unsafe && (
                <>
                    <Disclosure id="market.mintUnsafe" />
                    <Blocked code="MintUnsafeFlagged">Deposits are blocked on this market.</Blocked>
                </>
            )}
            {!signer && <div className="muted">Connect a wallet to lend this token.</div>}
            {signer &&
                (accounts.length > 0 && source ? (
                    <AccountSelect
                        label="From account"
                        value={source.address}
                        options={accounts.map((a) => ({
                            address: a.address,
                            label: accountLabel(a, chain.ata, decimals, symbol),
                        }))}
                        onChange={setSourceAddr}
                        testId={T.lendTokenAccount}
                    />
                ) : (
                    <div className="muted">You hold no {symbol} in this wallet.</div>
                ))}
            {guard && source && (
                <div
                    className="lend-notice"
                    data-testid={T.lendCpiGuardNotice}
                    data-state={bracket ? 'bracket' : 'blocked'}
                >
                    <Disclosure id="token.cpiGuard" />
                    <AckTarget id={bracketId}>
                        <Checkbox
                            checked={bracket}
                            onChange={(v) => setBracketFor(v ? source.address : null)}
                            testId={T.lendCpiGuardBracket}
                        >
                            Turn CPI Guard off for this deposit only (off → deposit → on, in one transaction).
                        </Checkbox>
                    </AckTarget>
                </div>
            )}
            <AckTarget id={ackId}>
                <Disclosure id="lender.solRepayment" acknowledged={ack} onAcknowledge={setAck} />
            </AckTarget>
            <AmountInput
                label="Amount"
                value={amountText}
                onChange={(t) => setAmountText(t)}
                decimals={decimals}
                unit={symbol}
                max={source?.amount}
                formatMax={(raw) => toInputText(raw, decimals)}
                error={overBalance ? 'Above the balance of this account.' : null}
                testId={T.lendAmountInput}
                maxTestId={T.lendMax}
                disabled={!signer || !source || unsafe}
                hint={source ? `Balance ${formatTokens(source.amount, decimals)} ${symbol}` : undefined}
            />
            <ToleranceInput
                label="Min shares tolerance"
                value={tolText}
                onChange={setTolText}
                invalid={tolBps === null}
            />
            {preview && !preview.ok && <RefusalLine refusal={preview.refusal} />}
            <Kv
                rows={[
                    [
                        'Shares minted',
                        preview?.ok ? (
                            <Tokens
                                key="r193"
                                raw={preview.value.shares}
                                decimals={decimals}
                                testId={T.lendPreview('shares')}
                            />
                        ) : (
                            '—'
                        ),
                    ],
                    [
                        'At least',
                        preview?.ok ? <Tokens key="r198" raw={preview.value.minShares} decimals={decimals} /> : '—',
                    ],
                    [
                        'Your share of the pool',
                        preview?.ok ? (
                            <Bps key="r201" value={preview.value.shareBps} testId={T.lendPreview('share-bps')} />
                        ) : (
                            '—'
                        ),
                    ],
                    [
                        'Token value',
                        preview?.ok ? (
                            <Tokens
                                key="r206"
                                raw={preview.value.value}
                                decimals={decimals}
                                symbol={symbol}
                                testId={T.lendPreview('value')}
                            />
                        ) : (
                            '—'
                        ),
                    ],
                    [
                        'Position after',
                        preview?.ok ? (
                            <Tokens key="r219" raw={preview.value.positionValue} decimals={decimals} symbol={symbol} />
                        ) : (
                            '—'
                        ),
                    ],
                    ...(poolUnpaid > 0n
                        ? ([
                              [
                                  'Unpaid rent in the pool',
                                  <Sol
                                      key="unpaid"
                                      lamports={unpaid?.receivable ?? poolUnpaid}
                                      frac={9}
                                      testId={T.lendPreview('unpaid-rent')}
                                  />,
                              ],
                              [
                                  'This deposit’s part if it is never paid',
                                  unpaid ? (
                                      <span key="unpaid-share" className="lend-unpaid" data-level={unpaid.level}>
                                          <Sol
                                              lamports={unpaid.exposure}
                                              frac={9}
                                              testId={T.lendPreview('unpaid-rent-share')}
                                          />
                                      </span>
                                  ) : (
                                      '—'
                                  ),
                              ],
                          ] as const)
                        : []),
                    ...(signer && !chain.lender && rent.data !== undefined
                        ? ([
                              ['New account rent (refunded on exit)', <Sol key="rent" lamports={rent.data} frac={6} />],
                          ] as const)
                        : []),
                ]}
            />
            {unpaid && unpaid.level !== 'none' && (
                <Disclosure
                    id="lender.unpaidRent"
                    vars={unpaidRentVarsOf(unpaid)}
                    collapsible={unpaid.level !== 'warn'}
                />
            )}
            {sweep && (
                <div className="banner" data-tone="info" role="status" data-testid={T.lendSweepNotice}>
                    This pool was wiped out. Your deposit first runs the permissionless post-wipe sweep (the venue
                    cashback claims and sweep_trader), so the old lenders receive what is left on the trader.
                </div>
            )}
            {buildError && (
                <div className="field-error" role="alert">
                    {buildError}
                </div>
            )}
            <FormFoot reason={blocker?.text ?? null} reasonTestId={T.lendDepositReason} reasonTarget={blocker?.target}>
                {!signer && <ConnectCta surface="lend" />}
                <Button
                    variant="primary"
                    block
                    busy={building}
                    disabled={!ready}
                    onClick={() => void submit()}
                    data-testid={T.lendSubmit}
                >
                    {guard && bracket ? 'Review deposit (with CPI Guard bracket)' : 'Review deposit'}
                </Button>
            </FormFoot>
            <LenderRisks owner={signer?.address ?? null} vars={lenderRiskVars(chain.config?.params)} />
            <Disclosure id="lender.lossHistory" vars={vars.lossHistory} collapsible />
            <Disclosure id="market.creatorSelfDealing" vars={vars.creatorSelfDealing} collapsible />
            <Disclosure id="lender.residuals" collapsible />
        </div>
    );
}

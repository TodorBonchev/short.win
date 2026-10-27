/** Borrower-delivered full debt, including during a live quote/routing outage. */
import type { Address } from '@solana/kit';
import { useState } from 'react';
import { Button } from '../../../components/Button.tsx';
import { Field, Select } from '../../../components/Field.tsx';
import { FormFoot } from '../../../components/FormFoot.tsx';
import { Sol, Tokens } from '../../../components/Num.tsx';
import { ErrorState, Skeleton } from '../../../components/States.tsx';
import { T } from '../../../config/testids.ts';
import { parseAmount } from '../../../lib/amount.ts';
import { shortAddress } from '../../../lib/format.ts';
import { TxReviewModal } from '../../../tx/TxReviewModal.tsx';
import { useWallet } from '../../../wallet/useWallet.ts';
import { firstReason, REASON, useConfirmedToast } from '../feedback.ts';
import { plainAmount } from '../ui.tsx';
import { repaymentReason } from './repay.ts';
import { useRepay } from './useRepay.ts';

export function RepayPanel({ position, symbol }: { position: Address; symbol: string }) {
    const { connected } = useWallet();
    const repay = useRepay(position);
    const [sourceAddress, setSourceAddress] = useState('');
    const [rentText, setRentText] = useState('');
    const parsed = parseAmount(rentText, 9, { allowZero: true });
    const maxRent = parsed.ok ? parsed.raw : null;
    const state = repay.state.data ?? null;
    const source =
        state?.sources.find((account) => account.address === sourceAddress) ??
        state?.sources.find((account) => !account.frozen && !account.cpiGuard && account.amount >= state.position.q) ??
        state?.sources[0] ??
        null;
    const stage = repay.flow.state.stage;
    const stale = state ? Date.now() - state.readAt > 15_000 : true;
    const reason = firstReason([
        [!connected, REASON.connect],
        [!connected?.signer, REASON.signer],
        [!state || !repay.math, 'Reading the position, token accounts and accrued rent'],
        [!!state && connected?.address !== state.position.borrower, REASON.notMine],
        [rentText === '', 'Enter the maximum accrued rent you authorize in SOL'],
        [!parsed.ok, 'Use a plain SOL amount with at most 9 decimal places'],
        [!!repay.previewError, repay.previewError ?? 'Rent preview unavailable'],
        [
            !!state &&
                !!connected &&
                repaymentReason(state, connected.address, source, maxRent, repay.preview) !== null,
            state && connected ? (repaymentReason(state, connected.address, source, maxRent, repay.preview) ?? '') : '',
        ],
        [stale, REASON.stale],
        [repay.building, REASON.building],
        [stage !== 'idle' && stage !== 'confirmed', REASON.inReview],
    ]);
    useConfirmedToast(repay.flow);
    return (
        <section className="stack" id="repay-tokens" aria-labelledby="repay-title" data-testid={T.repayPanel}>
            <h3 id="repay-title" className="short-section-title" tabIndex={-1}>
                Repay with tokens
            </h3>
            <p className="short-note">
                Return the full token debt from your wallet to close this position. You can repay while swaps or live
                price quotes are unavailable. Rent still accrues under the position’s terms, and token transfers must
                pass simulation.
            </p>
            {repay.state.error ? (
                <ErrorState error={repay.state.error} retry={() => void repay.state.refetch()} />
            ) : !state ? (
                connected ? (
                    <Skeleton width={240} />
                ) : (
                    <p className="short-note">Connect the borrower wallet to read repayment balances.</p>
                )
            ) : (
                <>
                    <dl className="kv">
                        <dt>Full token debt</dt>
                        <dd data-testid={T.repayQ} data-value={state.position.q.toString()}>
                            <Tokens raw={state.position.q} decimals={state.mintInfo.decimals} symbol={symbol} />
                        </dd>
                        <dt>Token mint</dt>
                        <dd className="num">{state.market.mint}</dd>
                        <dt>Estimated accrued rent</dt>
                        <dd data-testid={T.repayRent} data-value={repay.preview?.rentDue.toString()}>
                            {repay.preview ? <Sol lamports={repay.preview.rentDue} frac={9} /> : '—'}
                        </dd>
                        <dt>Estimated vault returned</dt>
                        <dd>
                            {repay.preview?.estimatedVaultReturn !== null &&
                            repay.preview?.estimatedVaultReturn !== undefined ? (
                                <Sol lamports={repay.preview.estimatedVaultReturn} frac={9} />
                            ) : (
                                '—'
                            )}
                        </dd>
                    </dl>
                    <Field
                        label="Token account"
                        hint="One account must hold the full debt. Frozen accounts and accounts with CPI Guard enabled cannot repay here."
                    >
                        {() => (
                            <Select
                                label="Repayment token account"
                                value={source?.address ?? ''}
                                onChange={setSourceAddress}
                                options={
                                    state.sources.length
                                        ? state.sources.map((account) => ({
                                              value: account.address,
                                              label: `${shortAddress(account.address)} · ${plainAmount(account.amount, state.mintInfo.decimals)} ${symbol}${account.frozen ? ' (frozen)' : account.cpiGuard ? ' (CPI Guard on)' : ''}`,
                                          }))
                                        : [{ value: '', label: 'No token accounts for this mint' }]
                                }
                                testId={T.repaySource}
                            />
                        )}
                    </Field>
                </>
            )}
            <Field
                label="Maximum accrued rent (SOL)"
                hint="This exact limit is checked on-chain. Rent may rise before landing; choose any extra room yourself. It cannot exceed the position vault."
                error={rentText !== '' && !parsed.ok ? 'Use a plain SOL amount with at most 9 decimal places.' : null}
            >
                {({ inputId, describedBy }) => (
                    <div className="input-wrap">
                        <input
                            id={inputId}
                            className="input num"
                            inputMode="decimal"
                            autoComplete="off"
                            value={rentText}
                            onChange={(event) => setRentText(event.target.value.replace(',', '.'))}
                            aria-describedby={describedBy}
                            aria-invalid={(rentText !== '' && !parsed.ok) || undefined}
                            data-testid={T.repayMaxRentInput}
                        />
                        <span className="input-suffix">SOL</span>
                    </div>
                )}
            </Field>
            {repay.preview?.vaultCanPay && (
                <Button
                    variant="ghost"
                    onClick={() => setRentText(plainAmount(repay.preview?.rentDue ?? 0n, 9))}
                    data-testid={T.repayUseEstimate}
                >
                    Use current rent estimate
                </Button>
            )}
            {maxRent !== null && (
                <p className="short-note" data-testid={T.repayExactLimit}>
                    Your exact rent limit: {plainAmount(maxRent, 9)} SOL ({maxRent.toString()} lamports).
                </p>
            )}
            {state && maxRent !== null && maxRent <= state.position.v && (
                <p className="short-note">
                    At your limit, at least {plainAmount(state.position.v - maxRent, 9)} SOL of the position vault
                    returns to you. Account rent refunds and the wallet’s network fee are shown in the transaction
                    review.
                </p>
            )}
            <p className="short-note" data-testid={T.repayPnlDisclosure}>
                The cost of obtaining tokens outside short.win is unknown. Returned SOL is not a profit figure; economic
                P&amp;L is unavailable for this repayment.
            </p>
            {!!(repay.buildError || repay.previewError || repay.mathError) && (
                <div className="banner" data-tone="danger" role="alert">
                    {repay.buildError ?? repay.previewError ?? 'The rent math module could not be loaded.'}
                </div>
            )}
            <FormFoot reason={reason} reasonTestId={T.repayReason}>
                <Button
                    variant="primary"
                    block
                    disabled={reason !== null}
                    busy={repay.building}
                    onClick={() => {
                        if (source && maxRent !== null) void repay.submit(source.address, maxRent, symbol);
                    }}
                    data-testid={T.repaySubmit}
                >
                    {!connected
                        ? 'Connect a wallet to repay'
                        : state && connected.address !== state.position.borrower
                          ? 'Not your position'
                          : 'Review token repayment'}
                </Button>
            </FormFoot>
            <TxReviewModal flow={repay.flow} />
        </section>
    );
}

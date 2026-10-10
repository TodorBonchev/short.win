/**
 * MARKETS track — create a market (FRONTEND.md §3.5): the mint, the safety verdict with every
 * failing rule (computed in the browser from the mint's bytes, FE-D5), venue detection and the
 * Raydium candidates (SHORT's venue layer), the cost (creation fee and rent), the `market.create`
 * disclosure, then `create_market` through the transaction pipeline: simulated before the wallet is
 * asked to sign (FE-D2). On confirmation the page opens the new market.
 *
 * M10-RESP: a token that cannot be listed replaces the cost with that verdict and has no Review
 * (D14); Review sits in a form foot with the reason it is disabled (sticky on a tablet, a bar above
 * the tab bar on a phone); one column under 1,024 px in the order mint → verdict → venue → pools
 * (cards) → cost; a Paste button for the mint under a coarse pointer.
 */
import type { Address } from '@solana/kit';
import { useQuery } from '@tanstack/react-query';
import { Link, useNavigate, useSearch } from '@tanstack/react-router';
import { useEffect, useId, useMemo, useRef, useState } from 'react';
import { qk } from '../../api/keys.ts';
import type { VenueName } from '../../api/types.ts';
import { AddressText } from '../../components/Address.tsx';
import { Badge, VenueBadge, venueLabel } from '../../components/Badge.tsx';
import { Button } from '../../components/Button.tsx';
import { type Column, DataTable } from '../../components/DataTable.tsx';
import { Disclosure } from '../../components/Disclosure.tsx';
import { addressInputProps, Field } from '../../components/Field.tsx';
import { FormFoot } from '../../components/FormFoot.tsx';
import { Sol } from '../../components/Num.tsx';
import { Panel } from '../../components/Panel.tsx';
import { ErrorState, Kv, Skeleton } from '../../components/States.tsx';
import { useToast } from '../../components/Toast.tsx';
import { PROGRAM_ERRORS, type ProgramErrorInfo } from '../../config/errors.ts';
import { T } from '../../config/testids.ts';
import { parseAddress } from '../../lib/address.ts';
import { formatBps, formatFixed, formatSol, shortAddress } from '../../lib/format.ts';
import { useMath } from '../../math/useMath.ts';
import { useAppClient } from '../../rpc/context.tsx';
import { TxReviewModal } from '../../tx/TxReviewModal.tsx';
import { useTxFlow } from '../../tx/useTxFlow.ts';
import { detectVenue, type PoolCandidate, raydiumCandidates, type VenueDetection } from '../../venues/index.ts';
import { useWallet } from '../../wallet/useWallet.ts';
import {
    buildCreateMarketPlan,
    type CostItem,
    creationAccounts,
    creationCost,
    type ListingChoice,
    listingVenue,
} from './create/createPlan.ts';
import {
    checkMintSafety,
    extensionName,
    extensionRejected,
    type MintVerdict,
    reasonCodes,
} from './create/mintSafety.ts';
import './markets.css';
import { useChainConfig, useChainMarket } from '../../rpc/queries.ts';
import { useRawAccount, useRentMinimums } from './shared/chain.ts';
import { useUrlText } from './shared/useUrlText.ts';

export type VerdictState = 'checking' | 'ok' | 'reject' | 'existing';

function errorByName(name: string): ProgramErrorInfo | undefined {
    for (const e of PROGRAM_ERRORS.values()) if (e.name === name) return e;
    return undefined;
}

const DETECTED_TEXT: Readonly<Record<VenueDetection['kind'], string>> = {
    none: 'No Pump.fun curve or PumpSwap pool. Auto lists it lend-only; an eligible Raydium pool below lists it for shorting.',
    pump_curve: 'On its Pump.fun bonding curve: it lists on the curve (Tier D).',
    pump_migrating: 'Its curve has completed and is migrating: it lists as migrating, and trading resumes on the pool.',
    pump_swap: 'Graduated to its canonical PumpSwap pool: it lists on PumpSwap (Tier C for the first 24 hours).',
};

function MintInfo({ v }: { v: MintVerdict }) {
    const b = v.base;
    return (
        <div data-testid={T.createMintInfo}>
            <Kv
                rows={[
                    [
                        'Token program',
                        v.program === 'token' ? 'SPL Token' : v.program === 'token-2022' ? 'Token-2022' : '—',
                    ],
                    ['Decimals', b ? String(b.decimals) : '—'],
                    [
                        'Supply',
                        b ? (
                            <span key="r77" className="num">
                                {formatFixed(b.supply, b.decimals, b.decimals)}
                            </span>
                        ) : (
                            '—'
                        ),
                    ],
                    [
                        'Mint authority',
                        b ? (
                            b.mintAuthority ? (
                                <AddressText key="r82" address={b.mintAuthority} copy={false} />
                            ) : (
                                'None (fixed supply)'
                            )
                        ) : (
                            '—'
                        ),
                    ],
                    [
                        'Freeze authority',
                        b ? (
                            b.freezeAuthority ? (
                                <AddressText key="r92" address={b.freezeAuthority} copy={false} />
                            ) : (
                                'None'
                            )
                        ) : (
                            '—'
                        ),
                    ],
                    [
                        'Extensions',
                        v.extensions.length === 0 ? (
                            'None'
                        ) : (
                            <span key="r99" className="mk-ext-list">
                                {v.extensions.map((t) => (
                                    <Badge key={t} tone={extensionRejected(v, t) ? 'danger' : 'ok'}>
                                        {extensionRejected(v, t) ? '✕ ' : '✓ '}
                                        {extensionName(t)}
                                    </Badge>
                                ))}
                            </span>
                        ),
                    ],
                ]}
            />
        </div>
    );
}

function Verdict({ state, verdict, mint }: { state: VerdictState; verdict: MintVerdict | null; mint: Address }) {
    return (
        <div className="stack">
            <div className="mk-verdict" data-testid={T.createVerdict} data-verdict={state} aria-live="polite">
                {state === 'checking' && (
                    <>
                        <Skeleton width={16} /> Checking the mint…
                    </>
                )}
                {state === 'ok' && <Badge tone="ok">✓ Passes the safety filter</Badge>}
                {state === 'reject' && (
                    <Badge tone="danger">✕ Cannot be listed: {verdict?.programError ?? 'rejected'}</Badge>
                )}
                {state === 'existing' && <Badge tone="info">A market already exists for this token</Badge>}
            </div>
            {state === 'existing' && (
                <Link
                    to="/m/$mint"
                    params={{ mint }}
                    className="btn"
                    data-variant="primary"
                    data-testid={T.createExisting}
                >
                    Open the market
                </Link>
            )}
            {state === 'reject' && verdict && (
                <ul className="mk-reasons">
                    {reasonCodes(verdict).map((code) => (
                        <li key={code} className="mk-reason" data-testid={T.createReason(code)}>
                            <div>{errorByName(code)?.message}</div>
                            {verdict.reasons
                                .filter((r) => r.code === code)
                                .map((r) => (
                                    <div key={r.detail} className="secondary">
                                        {r.detail}
                                    </div>
                                ))}
                            <span className="mk-reason-code">{code}</span>
                        </li>
                    ))}
                </ul>
            )}
            {verdict && state !== 'checking' && verdict.program !== null && <MintInfo v={verdict} />}
        </div>
    );
}

function candidateColumns(choiceKey: string, onChoose: (k: string) => void, name: string): Column<PoolCandidate>[] {
    return [
        {
            key: 'pool',
            header: 'Pool',
            card: 'title',
            render: (c) => (
                <label className="row mk-pool-choice">
                    <input
                        type="radio"
                        name={name}
                        value={c.pool}
                        checked={choiceKey === c.pool}
                        disabled={!c.eligible}
                        data-testid={T.createChoice(c.pool)}
                        onChange={() => onChoose(c.pool)}
                    />
                    <span className="num" title={c.pool}>
                        {shortAddress(c.pool)}
                    </span>
                </label>
            ),
        },
        { key: 'kind', header: 'Kind', card: 'aside', render: (c) => venueLabel(c.kind) },
        {
            key: 'sol',
            header: 'Real SOL',
            align: 'right',
            render: (c) => <Sol lamports={c.solReserve} frac={2} />,
        },
        {
            key: 'depth',
            header: 'Locked depth D',
            align: 'right',
            render: (c) => (
                <span className="num" data-value={c.lockedDepth.toString()}>
                    {c.lockedDepth.toLocaleString('en-US')}
                </span>
            ),
        },
        {
            key: 'lambda',
            header: 'Locked λ',
            align: 'right',
            render: (c) => <span className="num">{formatBps(c.lambdaBps, 1)}</span>,
        },
        {
            key: 'fee',
            header: 'Fee',
            align: 'right',
            render: (c) => (
                <span className="num" data-value={String(c.feePpm)}>
                    {formatFixed(BigInt(c.feePpm), 4, 3)}%
                </span>
            ),
        },
        {
            key: 'eligible',
            header: 'Eligible',
            render: (c) =>
                c.eligible ? (
                    <Badge tone="ok">Eligible</Badge>
                ) : (
                    <span className="stack">
                        {c.reasons.map((r) => (
                            <span key={r} title={errorByName(r)?.message}>
                                <Badge tone="danger">{r}</Badge>{' '}
                                <span className="secondary">{errorByName(r)?.message}</span>
                            </span>
                        ))}
                    </span>
                ),
        },
    ];
}

function Candidates({
    candidates,
    choiceKey,
    onChoose,
    name,
}: {
    candidates: readonly PoolCandidate[];
    choiceKey: string;
    onChoose: (k: string) => void;
    /** The radio group shared with the Auto choice. */
    name: string;
}) {
    if (candidates.length === 0) {
        return <p className="muted">No Raydium AMM v4 or CPMM SOL pool was found for this token.</p>;
    }
    return (
        <DataTable
            caption="Raydium SOL pools of this token"
            testId={T.createPools}
            className="mk-tbl mk-pools-tbl"
            columns={candidateColumns(choiceKey, onChoose, name)}
            rows={candidates}
            rowKey={(c) => c.pool}
            rowTestId={(c) => T.createPoolRow(c.pool)}
            rowAttrs={(c) => ({ 'data-eligible': String(c.eligible) })}
        />
    );
}

function CostBreakdown({
    items,
    rents,
    fee,
    total,
}: {
    items: readonly CostItem[];
    rents: readonly bigint[];
    fee: bigint;
    total: bigint;
}) {
    return (
        <div className="stack">
            <Kv
                rows={[
                    ['Creation fee', <Sol key="fee" lamports={fee} frac={6} />],
                    ...items.map(
                        (it, i) =>
                            [
                                `${it.label} rent${it.key === 'trader-ata' || it.key === 'trader-uva' ? ' (if new)' : ''}`,
                                <Sol key={it.key} lamports={rents[i] ?? 0n} frac={6} />,
                            ] as const,
                    ),
                ]}
            />
            <div className="row">
                <span className="grow secondary">Total</span>
                <span className="mk-cost-total" data-testid={T.createCost} data-value={total.toString()}>
                    <Sol lamports={total} frac={6} />
                </span>
            </div>
        </div>
    );
}

export function CreateMarketPage() {
    const search = useSearch({ from: '/markets/new' });
    const navigate = useNavigate({ from: '/markets/new' });
    const { rpc } = useAppClient();
    const toast = useToast();
    const { connected } = useWallet();
    const flow = useTxFlow();
    const [text, setText] = useUrlText(
        search.mint ?? '',
        (m) => void navigate({ search: m === '' ? {} : { mint: m }, replace: true }),
        (t) => parseAddress(t) ?? '',
    );
    const [choiceKey, setChoiceKey] = useState<string>('auto');
    const [building, setBuilding] = useState(false);
    const [buildError, setBuildError] = useState<string | null>(null);
    const mint = parseAddress(text);

    const account = useRawAccount(mint);
    const existing = useChainMarket(mint);
    const { math, error: mathError } = useMath();
    const verdict: MintVerdict | null = useMemo(
        () =>
            math && mint && account.isSuccess
                ? checkMintSafety(math, mint, account.data?.owner ?? null, account.data?.data ?? null)
                : null,
        [math, mint, account.isSuccess, account.data],
    );
    const state: VerdictState | null = !mint
        ? null
        : existing.data?.exists
          ? 'existing'
          : account.isPending || existing.isPending
            ? 'checking'
            : verdict && !verdict.ok
              ? 'reject'
              : verdict?.ok && existing.isSuccess
                ? 'ok'
                : 'checking';

    const detection = useQuery({
        queryKey: [...qk.venueAccounts(mint ?? ''), 'detect'],
        queryFn: () => detectVenue(rpc, mint as Address),
        enabled: state === 'ok',
        retry: false,
        staleTime: 5_000,
    });
    const nonPump = detection.data?.kind === 'none' && detection.data.refusal === null;
    const candidates = useQuery({
        queryKey: [...qk.venueAccounts(mint ?? ''), 'raydium'],
        queryFn: () => raydiumCandidates(rpc, mint as Address),
        enabled: state === 'ok' && nonPump,
        retry: false,
        staleTime: 5_000,
    });

    const chosenPool = candidates.data?.find((c) => c.pool === choiceKey && c.eligible) ?? null;
    const choice: ListingChoice | null =
        state !== 'ok' || !detection.data || detection.data.refusal !== null
            ? null
            : chosenPool
              ? { kind: 'raydium', candidate: chosenPool }
              : { kind: 'auto', detection: detection.data };
    const venue: VenueName | null = choice ? listingVenue(choice) : null;

    const configQ = useChainConfig();
    const config = configQ.data?.exists ? configQ.data.data : null;
    const program = verdict?.program ?? 'token';
    const items = venue ? creationAccounts(venue, program) : [];
    const rents = useRentMinimums(
        items.map((i) => i.size),
        venue !== null,
    );
    const cost = config && rents.data ? creationCost(config.params.marketCreationFee, rents.data) : null;

    // On confirmation: the new market's page (FRONTEND.md §3.5 step 5).
    const confirmedFor = useRef<string | null>(null);
    useEffect(() => {
        if (flow.state.stage === 'confirmed' && mint && confirmedFor.current !== mint) {
            confirmedFor.current = mint;
            toast.push({ kind: 'success', title: 'Market created', body: shortAddress(mint, 6, 6) });
            void navigate({ to: '/m/$mint', params: { mint } });
        }
    }, [flow.state.stage, mint, navigate, toast]);

    const blocked: string | null = !connected?.signer
        ? 'Connect a wallet that can sign to create the market.'
        : state !== 'ok'
          ? null
          : detection.isError
            ? 'Venue detection failed.'
            : detection.data?.refusal
              ? 'This token cannot be listed (see the venue above).'
              : configQ.isSuccess && !config
                ? 'The protocol configuration is not initialized on this cluster.'
                : configQ.isError || rents.isError
                  ? 'The creation fee or the rent could not be read from the chain. Reload to try again.'
                  : !choice || !cost
                    ? 'Reading the venue and the cost…'
                    : null;
    const canSubmit = state === 'ok' && choice !== null && cost !== null && !!connected?.signer && blocked === null;
    // The token cannot be listed: the safety filter's verdict, or the venue's refusal (D14).
    const refusal = state === 'ok' ? (detection.data?.refusal ?? null) : null;
    const cannotList = state === 'reject' || refusal !== null;
    const cannotListWhy =
        state === 'reject'
            ? (verdict && errorByName(reasonCodes(verdict)[0] ?? '')?.message) || 'It fails the safety filter.'
            : refusal
              ? (errorByName(refusal)?.message ?? refusal)
              : null;
    // Why Review is disabled, from the same predicates as `canSubmit` (FormFoot).
    const reason: string | null = canSubmit
        ? null
        : !mint
          ? 'Enter the token’s mint address first.'
          : state === 'existing'
            ? 'This token already has a market.'
            : state === 'checking'
              ? 'Checking the mint…'
              : (blocked ?? 'Reading the venue and the cost…');
    const choiceGroup = useId();

    const submit = async () => {
        if (!canSubmit || !mint || !choice || !cost || !config || !connected?.signer || !verdict) return;
        setBuilding(true);
        setBuildError(null);
        try {
            const plan = await buildCreateMarketPlan({
                signer: connected.signer,
                mint,
                tokenProgram: account.data?.owner as Address,
                choice,
                creationFee: config.params.marketCreationFee,
                cost,
                label: shortAddress(mint),
            });
            flow.start(plan);
        } catch (e) {
            setBuildError(e instanceof Error ? e.message : String(e));
        } finally {
            setBuilding(false);
        }
    };

    return (
        <div className="page" data-testid={T.page('create-market')}>
            <div className="page-head">
                <h1>Create market</h1>
                <span className="muted">Listing is permissionless: any token that passes the safety filter.</span>
            </div>
            <div className="mk-create">
                <div className="stack">
                    <Panel title="Token">
                        <div className="stack">
                            <Field
                                label="Mint address"
                                hint="The token is identified by its mint only; names and tickers can be copied."
                                error={text.trim() !== '' && !mint ? 'Enter a base58 mint address.' : null}
                            >
                                {({ inputId, describedBy }) => (
                                    <div
                                        className="input-wrap"
                                        data-invalid={text.trim() !== '' && !mint ? 'true' : undefined}
                                    >
                                        <input
                                            {...addressInputProps}
                                            id={inputId}
                                            className="input num"
                                            value={text}
                                            placeholder="Mint address"
                                            aria-describedby={describedBy}
                                            aria-invalid={text.trim() !== '' && !mint ? true : undefined}
                                            data-testid={T.createMintInput}
                                            onChange={(e) => {
                                                setText(e.target.value);
                                                setChoiceKey('auto');
                                            }}
                                        />
                                        <button
                                            type="button"
                                            className="btn mk-paste"
                                            data-variant="ghost"
                                            data-testid={T.createMintPaste}
                                            onClick={() => {
                                                // A tap is the user gesture the clipboard needs; a refusal changes nothing.
                                                navigator.clipboard
                                                    ?.readText()
                                                    .then((t) => {
                                                        if (t.trim() === '') return;
                                                        setText(t.trim());
                                                        setChoiceKey('auto');
                                                    })
                                                    .catch(() => undefined);
                                            }}
                                        >
                                            Paste
                                        </button>
                                    </div>
                                )}
                            </Field>
                            {mint &&
                                state &&
                                (account.isError || existing.isError ? (
                                    <ErrorState
                                        error={account.error ?? existing.error}
                                        retry={() => {
                                            void account.refetch();
                                            void existing.refetch();
                                        }}
                                    />
                                ) : mathError && !math && state === 'checking' ? (
                                    // The verdict is the WASM facade's (`mintCheck`, FE-15): no module, no verdict.
                                    <ErrorState error={mathError} />
                                ) : (
                                    <Verdict state={state} verdict={verdict} mint={mint} />
                                ))}
                        </div>
                    </Panel>
                    {state === 'ok' && (
                        <Panel title="Venue">
                            <div className="stack">
                                {detection.isPending ? (
                                    <span className="row" data-testid={T.createVenue}>
                                        <Skeleton width={120} /> Detecting the venue…
                                    </span>
                                ) : detection.isError ? (
                                    <div data-testid={T.createVenue}>
                                        <ErrorState error={detection.error} retry={() => void detection.refetch()} />
                                    </div>
                                ) : detection.data ? (
                                    <>
                                        <div
                                            className="row"
                                            data-testid={T.createVenue}
                                            data-value={detection.data.kind}
                                        >
                                            <VenueBadge venue={detection.data.kind} />
                                            <span className="secondary">{DETECTED_TEXT[detection.data.kind]}</span>
                                        </div>
                                        {detection.data.refusal && (
                                            <div
                                                className="mk-reason"
                                                data-testid={T.createReason(detection.data.refusal)}
                                            >
                                                <div>{errorByName(detection.data.refusal)?.message}</div>
                                                <span className="mk-reason-code">{detection.data.refusal}</span>
                                            </div>
                                        )}
                                        {detection.data.refusal === null && (
                                            <label className="mk-choice">
                                                <input
                                                    type="radio"
                                                    name={choiceGroup}
                                                    value="auto"
                                                    checked={chosenPool === null}
                                                    data-testid={T.createChoice('auto')}
                                                    onChange={() => setChoiceKey('auto')}
                                                />
                                                <span>
                                                    Auto: list as <strong>{venueLabel(detection.data.kind)}</strong>
                                                </span>
                                            </label>
                                        )}
                                        {nonPump && (
                                            <section aria-label="Raydium pools" className="stack">
                                                <h3>Raydium pools</h3>
                                                {candidates.isPending ? (
                                                    <Skeleton width={200} />
                                                ) : candidates.isError ? (
                                                    <ErrorState
                                                        error={candidates.error}
                                                        retry={() => void candidates.refetch()}
                                                    />
                                                ) : (
                                                    <Candidates
                                                        candidates={candidates.data ?? []}
                                                        choiceKey={chosenPool ? chosenPool.pool : 'auto'}
                                                        onChoose={setChoiceKey}
                                                        name={choiceGroup}
                                                    />
                                                )}
                                            </section>
                                        )}
                                    </>
                                ) : null}
                            </div>
                        </Panel>
                    )}
                </div>
                <Panel title="Cost" className="mk-create-cost">
                    <div className="stack">
                        {cannotList ? (
                            <div className="mk-cannot" role="status">
                                <Badge tone="danger">Cannot be listed</Badge>
                                <p>
                                    <strong>This token cannot be listed.</strong> {cannotListWhy}
                                </p>
                            </div>
                        ) : venue && cost ? (
                            <CostBreakdown items={items} rents={rents.data ?? []} fee={cost.fee} total={cost.total} />
                        ) : (
                            <p className="muted">
                                The cost appears once the token passes the check and its venue is known.
                            </p>
                        )}
                        {cost && !cannotList && (
                            <Disclosure
                                id="market.create"
                                vars={{ fee: `${formatSol(cost.fee, 6)} SOL`, rent: `${formatSol(cost.rent, 6)} SOL` }}
                            />
                        )}
                        <Disclosure id="market.venueTrust" collapsible />
                        {buildError && (
                            <p className="field-error" role="alert">
                                {buildError}
                            </p>
                        )}
                        {!cannotList && (
                            <div className="mk-create-foot">
                                <FormFoot reason={reason} reasonTestId={T.createSubmitReason}>
                                    <Button
                                        variant="primary"
                                        block
                                        disabled={!canSubmit}
                                        busy={building}
                                        data-testid={T.createSubmit}
                                        onClick={() => void submit()}
                                    >
                                        {venue ? `Review: list as ${venueLabel(venue)}` : 'Review'}
                                    </Button>
                                </FormFoot>
                            </div>
                        )}
                    </div>
                </Panel>
            </div>
            <TxReviewModal flow={flow} />
        </div>
    );
}

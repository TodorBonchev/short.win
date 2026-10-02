/** Small building blocks of the lend panel (LEND track; composed from the scaffold's components). */
import { type ReactNode, useId } from 'react';
import { Field } from '../../components/Field.tsx';
import { T } from '../../config/testids.ts';
import { formatTokens, shortAddress } from '../../lib/format.ts';
import type { TokenAccountInfo } from '../../tx/tokenAccount.ts';
import type { Refusal } from './previews.ts';

/** Why an action is refused now (`data-code` = the program error name). */
export function Blocked({ code, children }: { code: string; children: ReactNode }) {
    return (
        <div
            className="banner lend-blocked"
            data-tone="warn"
            role="status"
            data-testid={T.lendBlocked}
            data-code={code}
        >
            <div>{children}</div>
        </div>
    );
}

/** A preview the program would refuse, with the mapped message (FRONTEND.md §5.9). */
export function RefusalLine({ refusal }: { refusal: Refusal }) {
    return (
        <Blocked code={refusal.name}>
            <strong>{refusal.name}</strong> {refusal.message}
        </Blocked>
    );
}

/** The tolerance below the quote (FE-D6: default 0.5 %, within [0.1 %, 50 %]). */
export function ToleranceInput({
    label,
    value,
    onChange,
    invalid,
}: {
    label: string;
    value: string;
    onChange: (v: string) => void;
    invalid: boolean;
}) {
    return (
        <Field label={label} error={invalid ? 'Use 0.1 to 50 (%).' : null}>
            {({ inputId, describedBy }) => (
                <div className="input-wrap lend-tolerance" data-invalid={invalid ? 'true' : undefined}>
                    <input
                        id={inputId}
                        className="input num"
                        inputMode="decimal"
                        autoComplete="off"
                        value={value}
                        aria-invalid={invalid || undefined}
                        aria-describedby={describedBy}
                        data-testid={T.lendSlippageInput}
                        onChange={(e) => onChange(e.target.value.replace(',', '.'))}
                    />
                    <span className="input-suffix">%</span>
                </div>
            )}
        </Field>
    );
}

export interface AccountOption {
    readonly address: string;
    readonly label: string;
}

/** A labelled native select of token accounts. */
export function AccountSelect({
    label,
    value,
    options,
    onChange,
    testId,
    hint,
    data,
}: {
    label: string;
    value: string;
    options: readonly AccountOption[];
    onChange: (v: string) => void;
    testId: string;
    hint?: ReactNode;
    data?: Readonly<Record<`data-${string}`, string>>;
}) {
    const id = useId();
    const hintId = useId();
    return (
        <div className="field">
            <label className="field-label" htmlFor={id}>
                <span>{label}</span>
            </label>
            <select
                id={id}
                className="select lend-select"
                value={value}
                data-testid={testId}
                aria-describedby={hint ? hintId : undefined}
                onChange={(e) => onChange(e.target.value)}
                {...data}
            >
                {options.map((o) => (
                    <option key={o.address} value={o.address}>
                        {o.label}
                    </option>
                ))}
            </select>
            {hint && (
                <div id={hintId} className="field-hint">
                    {hint}
                </div>
            )}
        </div>
    );
}

/** "Main (ATA) · 1,234.5 BONK · CPI Guard" */
export function accountLabel(a: TokenAccountInfo, ata: string | null, decimals: number, symbol: string): string {
    const name = a.address === ata ? 'Main (ATA)' : shortAddress(a.address);
    const flags = [a.cpiGuard ? 'CPI Guard' : null, a.memoRequired ? 'memo required' : null].filter(Boolean);
    return `${name} · ${formatTokens(a.amount, decimals)} ${symbol}${flags.length ? ` · ${flags.join(' · ')}` : ''}`;
}

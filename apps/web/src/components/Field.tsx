/**
 * Form inputs (FRONTEND.md §7.3, §9.5). `AmountInput` keeps the user's text and reports the
 * parsed raw `bigint` (strict: `lib/amount.ts`), with a unit suffix and a Max button inside the
 * field's right edge (a 44 px hit under a coarse pointer).
 */
import { type InputHTMLAttributes, type ReactNode, useId } from 'react';
import { glossaryFor } from '../config/glossary.ts';
import { AMOUNT_ERROR_TEXT, type ParsedAmount, parseAmount } from '../lib/amount.ts';
import { GlossaryTip } from './Glossary.tsx';

/**
 * For an input that takes a base58 address (a mint, a wallet) or a ticker (FRONTEND.md §7.6): no
 * capitalisation, autocorrection, spell check or autofill — a phone keyboard would otherwise change
 * the case of a case-sensitive mint. Spread it, then override `enterKeyHint` where it differs.
 */
export const addressInputProps = {
    autoCapitalize: 'none',
    autoCorrect: 'off',
    spellCheck: false,
    autoComplete: 'off',
    inputMode: 'text',
    enterKeyHint: 'go',
} as const satisfies InputHTMLAttributes<HTMLInputElement>;

export interface FieldProps {
    readonly label: ReactNode;
    readonly hint?: ReactNode;
    readonly error?: string | null;
    readonly aside?: ReactNode;
    readonly children: (ids: { inputId: string; describedBy: string | undefined }) => ReactNode;
}

export function Field({ label, hint, error, aside, children }: FieldProps) {
    const inputId = useId();
    const hintId = useId();
    const errId = useId();
    const describedBy = [hint ? hintId : null, error ? errId : null].filter(Boolean).join(' ') || undefined;
    // A glossary label's ⓘ sits beside the <label>, not in it: the input's name stays the label.
    const term = glossaryFor(label);
    const labelEl = (
        <label className="field-label" htmlFor={inputId}>
            <span>{label}</span>
            {aside}
        </label>
    );
    return (
        <div className="field">
            {term ? (
                <div className="field-head">
                    {labelEl}
                    <GlossaryTip id={term} />
                </div>
            ) : (
                labelEl
            )}
            {children({ inputId, describedBy })}
            {hint && !error && (
                <div id={hintId} className="field-hint">
                    {hint}
                </div>
            )}
            {error && (
                <div id={errId} className="field-error" role="alert">
                    {error}
                </div>
            )}
        </div>
    );
}

export interface AmountInputProps {
    readonly label: ReactNode;
    readonly value: string;
    readonly onChange: (text: string, parsed: ParsedAmount) => void;
    readonly decimals: number;
    readonly unit?: string;
    readonly hint?: ReactNode;
    /** Raw maximum; shows a Max button that fills it. */
    readonly max?: bigint;
    readonly formatMax?: (raw: bigint) => string;
    /** An extra validation message (e.g. "above your balance"). */
    readonly error?: string | null;
    readonly testId?: string;
    readonly maxTestId?: string;
    readonly disabled?: boolean;
}

export function AmountInput({
    label,
    value,
    onChange,
    decimals,
    unit,
    hint,
    max,
    formatMax,
    error,
    testId,
    maxTestId,
    disabled,
}: AmountInputProps) {
    const parsed = parseAmount(value, decimals);
    const own = value !== '' && !parsed.ok ? AMOUNT_ERROR_TEXT[parsed.error] : null;
    const shown = own ?? error ?? null;
    return (
        <Field label={label} hint={hint} error={shown}>
            {({ inputId, describedBy }) => (
                <div className="input-wrap" data-invalid={shown ? 'true' : undefined}>
                    <input
                        id={inputId}
                        className="input num"
                        inputMode="decimal"
                        enterKeyHint="done"
                        autoComplete="off"
                        spellCheck={false}
                        placeholder="0.00"
                        value={value}
                        disabled={disabled}
                        aria-invalid={shown ? true : undefined}
                        aria-describedby={describedBy}
                        data-testid={testId}
                        onChange={(e) => {
                            const text = e.target.value.replace(',', '.');
                            onChange(text, parseAmount(text, decimals));
                        }}
                    />
                    {unit && <span className="input-suffix">{unit}</span>}
                    {max !== undefined && formatMax && (
                        <button
                            type="button"
                            className="btn input-max hit"
                            data-variant="ghost"
                            data-size="sm"
                            data-testid={maxTestId}
                            disabled={disabled}
                            onClick={() => {
                                const text = formatMax(max);
                                onChange(text, parseAmount(text, decimals));
                            }}
                        >
                            Max
                        </button>
                    )}
                </div>
            )}
        </Field>
    );
}

export function Checkbox({
    checked,
    onChange,
    children,
    testId,
    disabled,
    id,
}: {
    checked: boolean;
    onChange: (v: boolean) => void;
    children: ReactNode;
    testId?: string;
    disabled?: boolean;
    /** For a FormFoot `reasonTarget` that focuses this box. */
    id?: string;
}) {
    return (
        <label className="checkbox">
            <input
                id={id}
                type="checkbox"
                checked={checked}
                disabled={disabled}
                data-testid={testId}
                onChange={(e) => onChange(e.target.checked)}
            />
            <span>{children}</span>
        </label>
    );
}

export function Select<V extends string>({
    label,
    value,
    options,
    onChange,
    testId,
}: {
    label: string;
    value: V;
    options: readonly { value: V; label: string }[];
    onChange: (v: V) => void;
    testId?: string;
}) {
    return (
        <select
            className="select"
            aria-label={label}
            value={value}
            data-testid={testId}
            onChange={(e) => onChange(e.target.value as V)}
        >
            {options.map((o) => (
                <option key={o.value} value={o.value}>
                    {o.label}
                </option>
            ))}
        </select>
    );
}

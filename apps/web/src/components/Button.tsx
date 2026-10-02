import type { ButtonHTMLAttributes } from 'react';

export type ButtonVariant = 'default' | 'primary' | 'short' | 'danger' | 'ghost';

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
    readonly variant?: ButtonVariant;
    readonly size?: 'sm' | 'md';
    readonly block?: boolean;
    readonly busy?: boolean;
}

/** A button. `busy` disables it and sets `aria-busy`. Defaults to `type="button"`. */
export function Button({
    variant = 'default',
    size = 'md',
    block,
    busy,
    disabled,
    type,
    children,
    ...rest
}: ButtonProps) {
    return (
        <button
            {...rest}
            type={type === 'submit' ? 'submit' : type === 'reset' ? 'reset' : 'button'}
            className={`btn ${rest.className ?? ''}`.trim()}
            data-variant={variant}
            data-size={size}
            data-block={block ? 'true' : undefined}
            disabled={disabled || busy}
            aria-busy={busy || undefined}
        >
            {children}
        </button>
    );
}

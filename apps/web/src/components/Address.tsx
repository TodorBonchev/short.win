import { useState } from 'react';
import { explorerAddress } from '../config/env.ts';
import { shortAddress } from '../lib/format.ts';
import { Reveal } from './Reveal.tsx';

/**
 * A shortened address with the full one in `title`, and a copy button. With `reveal` the short
 * address is a button that shows the full address, Copy and Explorer (FRONTEND.md §7.6).
 */
export function AddressText({
    address,
    testId,
    copy = true,
    reveal,
}: {
    address: string;
    testId?: string;
    copy?: boolean;
    reveal?: boolean;
}) {
    const [copied, setCopied] = useState(false);
    const text = (
        <span className="num" title={address} data-testid={testId} data-value={address}>
            {shortAddress(address)}
        </span>
    );
    return (
        <span className="address-text">
            {reveal ? (
                <Reveal value={address} kind="address" label="Address" explorer={explorerAddress(address)}>
                    {text}
                </Reveal>
            ) : (
                text
            )}
            {copy && (
                <button
                    type="button"
                    className="btn hit"
                    data-variant="ghost"
                    data-size="sm"
                    aria-label={copied ? 'Copied' : `Copy ${address}`}
                    onClick={() => {
                        void navigator.clipboard?.writeText(address).then(() => {
                            setCopied(true);
                            setTimeout(() => setCopied(false), 1_200);
                        });
                    }}
                >
                    {copied ? '✓' : '⧉'}
                </button>
            )}
        </span>
    );
}

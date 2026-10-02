import { useState } from 'react';
import { shortAddress } from '../lib/format.ts';

/** A shortened address with the full one in `title`, and a copy button. */
export function AddressText({ address, testId, copy = true }: { address: string; testId?: string; copy?: boolean }) {
    const [copied, setCopied] = useState(false);
    return (
        <span className="row" style={{ gap: 4, display: 'inline-flex' }}>
            <span className="num" title={address} data-testid={testId} data-value={address}>
                {shortAddress(address)}
            </span>
            {copy && (
                <button
                    type="button"
                    className="btn"
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

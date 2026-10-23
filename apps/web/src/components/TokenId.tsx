/**
 * A token's identity on screen (FE-D8; M9-SEC-5): the (normalised, isolated) symbol next to the
 * short mint — the mint is the identity, the symbol only a hint — with the full mint in `title`.
 * With `reveal` the short mint is a button that shows the full mint, Copy and Explorer (a touch
 * screen has no `title`; FRONTEND.md §7.6) — not inside a row that opens on a tap.
 */
import { displayTokenMetadata } from '../config/devnetSmokeToken.ts';
import { config, explorerAddress } from '../config/env.ts';
import { cleanSymbol } from '../lib/displayText.ts';
import { shortAddress } from '../lib/format.ts';
import { Reveal } from './Reveal.tsx';

export function TokenId({
    symbol,
    mint,
    testId,
    className,
    reveal,
}: {
    symbol: string | null | undefined;
    mint: string;
    testId?: string;
    className?: string;
    reveal?: boolean;
}) {
    const sym = cleanSymbol(displayTokenMetadata(config.cluster, mint, symbol, null).symbol);
    const short = shortAddress(mint, 4, 4);
    return (
        <span className={`token-id ${className ?? ''}`.trim()} title={mint} data-testid={testId} data-value={mint}>
            {sym && sym !== short && <bdi className="token-symbol">{sym}</bdi>}
            {reveal ? (
                <Reveal value={mint} kind="mint" label="Mint" explorer={explorerAddress(mint)}>
                    <span className="num muted token-mint">{short}</span>
                </Reveal>
            ) : (
                <span className="num muted token-mint">{short}</span>
            )}
        </span>
    );
}

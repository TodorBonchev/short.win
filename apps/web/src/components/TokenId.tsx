/**
 * A token's identity on screen (FE-D8; M9-SEC-5): the (normalised, isolated) symbol next to the
 * short mint — the mint is the identity, the symbol only a hint — with the full mint in `title`.
 */
import { cleanSymbol } from '../lib/displayText.ts';
import { shortAddress } from '../lib/format.ts';

export function TokenId({
    symbol,
    mint,
    testId,
    className,
}: {
    symbol: string | null | undefined;
    mint: string;
    testId?: string;
    className?: string;
}) {
    const sym = cleanSymbol(symbol);
    const short = shortAddress(mint, 4, 4);
    return (
        <span className={`token-id ${className ?? ''}`.trim()} title={mint} data-testid={testId} data-value={mint}>
            {sym && sym !== short && <bdi className="token-symbol">{sym}</bdi>}
            <span className="num muted token-mint">{short}</span>
        </span>
    );
}

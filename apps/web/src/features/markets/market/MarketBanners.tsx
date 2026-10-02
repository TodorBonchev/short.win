/**
 * The market page's banners in FRONTEND.md §3.4's severity order, each a verbatim disclosure
 * (§4); paused opens show the program's own message for `OpensPaused` (one copy of every text).
 */
import type { GlobalConfig } from '@shortwin/client';
import { Disclosure } from '../../../components/Disclosure.tsx';
import { routeBrokenVars } from '../../../config/disclosureVars.ts';
import { programError } from '../../../config/errors.ts';
import { T } from '../../../config/testids.ts';
import { type MarketFlagKey, marketBanners } from '../shared/marketFlags.ts';

const OPENS_PAUSED = 6050;

export function MarketBanners({
    flags,
    config,
}: {
    flags: readonly MarketFlagKey[];
    /** The live `GlobalConfig` (governable numbers in the copy, M9-UX-3); `null` while unread. */
    config?: GlobalConfig | null;
}) {
    const banners = marketBanners(flags);
    if (banners.length === 0) return null;
    return (
        <div className="mk-banners">
            {banners.map((b) => (
                <div key={b.key} className="mk-banner" data-testid={T.marketBanner(b.key)} data-severity={b.severity}>
                    {b.disclosure ? (
                        <Disclosure id={b.disclosure} vars={routeBrokenVars(config?.params)} />
                    ) : (
                        <div className="banner" data-tone="warn" role="status">
                            {programError(OPENS_PAUSED)?.message}
                        </div>
                    )}
                </div>
            ))}
        </div>
    );
}

/**
 * O-2 (the owner's thirteenth round; audit finding D-23): on a coverage-paused market the page's
 * banner shows `market.coveragePause` in full, so the Short panel links to it instead of repeating
 * it. In the trade sheet the banner is behind the sheet: the link says so, closes the sheet and then
 * focuses the banner (`PageBanners.focus`, the market page). Without that banner (the 5 s after a
 * clear, or the indexer's flags missing while the chain says paused) there is no link, and the
 * panel keeps the disclosure (`ShortPanel`).
 */
import { usePageBanners } from '../../components/PageBanners.tsx';
import { copy } from '../../config/copy.ts';
import { T } from '../../config/testids.ts';

export function CoverageLink() {
    const banners = usePageBanners();
    if (!banners?.shown.has('coverage-paused')) return null;
    return (
        <button
            type="button"
            className="link-button short-coverage-link"
            data-testid={T.shortCoverageLink}
            onClick={() => banners.focus('coverage-paused')}
        >
            {copy(banners.inSheet ? 'coverage.link.sheet' : 'coverage.link.inline')}
        </button>
    );
}

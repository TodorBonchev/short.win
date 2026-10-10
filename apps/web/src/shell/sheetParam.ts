/**
 * A sheet in the URL (FRONTEND.md §7.6): `?sheet=trade` on a market, `?sheet=add-margin|close` on a
 * position. Opening pushes a history entry marked `state.sheet`, so the browser's / the OS's Back
 * closes the sheet; closing goes back when this entry was pushed by opening it, and otherwise (a
 * deep link, a reload) replaces the URL without `sheet` — never leaving the page.
 */
import { useNavigate, useRouter, useSearch } from '@tanstack/react-router';
import type { MarketSheet, PositionSheet } from '../router.tsx';

type SheetRoute = '/m/$mint' | '/positions/$position';
type SheetValue<R extends SheetRoute> = R extends '/m/$mint' ? MarketSheet : PositionSheet;

/** How a close leaves the sheet: `back` when opening it pushed this entry, else `replace`. */
export function closeMode(historyState: { sheet?: boolean } | undefined): 'back' | 'replace' {
    return historyState?.sheet ? 'back' : 'replace';
}

export function useSheetParam<R extends SheetRoute>(
    from: R,
): {
    readonly open: SheetValue<R> | undefined;
    openSheet(value: SheetValue<R>, extraSearch?: Readonly<Record<string, unknown>>): void;
    closeSheet(): void;
} {
    // Not strict: the hook serves two routes; `from` types the value.
    const raw: unknown = useSearch({ strict: false });
    const search = raw as { sheet?: SheetValue<R> };
    void from;
    const navigate = useNavigate();
    const router = useRouter();
    return {
        open: search.sheet,
        openSheet(value, extraSearch = {}) {
            void navigate({
                to: '.',
                search: ((s: Record<string, unknown>) => ({ ...s, ...extraSearch, sheet: value })) as never,
                state: (prev) => ({ ...prev, sheet: true }),
            });
        },
        closeSheet() {
            if (closeMode(router.state.location.state) === 'back') router.history.back();
            else
                void navigate({
                    to: '.',
                    search: ((s: Record<string, unknown>) => ({ ...s, sheet: undefined })) as never,
                    replace: true,
                });
        },
    };
}

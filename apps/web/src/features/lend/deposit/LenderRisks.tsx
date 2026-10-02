/**
 * `lender.risks` on the deposit form (FRONTEND.md §4.2): always shown — expanded the first time a
 * wallet sees it, collapsible afterwards (remembered per wallet in this browser only; a missing or
 * blocked storage shows it expanded).
 */
import { useEffect, useMemo } from 'react';
import { Disclosure } from '../../../components/Disclosure.tsx';

const KEY = 'shortwin:lend:risks-seen:';

function wasSeen(key: string): boolean {
    try {
        return globalThis.localStorage?.getItem(key) === '1';
    } catch {
        return false;
    }
}

export function LenderRisks({ owner, vars }: { owner: string | null; vars: Readonly<Record<string, string>> }) {
    const key = `${KEY}${owner ?? 'anon'}`;
    const seen = useMemo(() => wasSeen(key), [key]);
    useEffect(() => {
        try {
            globalThis.localStorage?.setItem(key, '1');
        } catch {
            // storage blocked: the disclosure stays expanded next time
        }
    }, [key]);
    return <Disclosure id="lender.risks" vars={vars} collapsible={seen} />;
}

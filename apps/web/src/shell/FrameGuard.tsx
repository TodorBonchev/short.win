/**
 * Clickjacking guard (FRONTEND.md §9.3; M9-SEC-6): the app never runs inside another page. The
 * static host sends `X-Frame-Options: DENY` and `frame-ancestors 'none'`, but a meta CSP cannot
 * carry `frame-ancestors` and a host may ignore `dist/_headers` — so `main.tsx` also refuses to
 * mount when framed and renders this notice instead: no wallet button, no disclosure checkbox, no
 * Sign button exists for a framing page to steer clicks onto.
 */
import { T } from '../config/testids.ts';

/** Whether this document runs inside a frame (a cross-origin `top` that throws counts as framed). */
export function isFramed(w: Pick<Window, 'top' | 'self'> = window): boolean {
    try {
        return w.top !== w.self;
    } catch {
        return true;
    }
}

export function FrameGuard({ href }: { href: string }) {
    return (
        <div className="empty" role="alert" data-testid={T.frameGuard}>
            <strong>ShortWin does not run inside another page.</strong>
            <p>
                Open{' '}
                <a href={href} target="_blank" rel="noopener noreferrer">
                    short.win
                </a>{' '}
                directly in its own tab, and check the address bar before you connect a wallet.
            </p>
        </div>
    );
}

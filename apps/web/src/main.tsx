import './styles/tokens.css';
import './styles/base.css';
import './styles/components.css';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { FrameGuard, isFramed } from './shell/FrameGuard.tsx';

const el = document.getElementById('root');
if (!el) throw new Error('#root missing');
const root = createRoot(el);

// The configuration is validated at import (config/env.ts); a bad one renders an error instead of a
// half-working app. Production builds fail earlier, in vite.config.ts.
function start(): void {
    import('./App.tsx')
        .then(({ App }) =>
            root.render(
                <StrictMode>
                    <App />
                </StrictMode>,
            ),
        )
        .catch((e: unknown) => {
            root.render(
                <div className="empty" role="alert">
                    <strong>ShortWin could not start.</strong>
                    <pre style={{ whiteSpace: 'pre-wrap', textAlign: 'left' }}>
                        {e instanceof Error ? e.message : String(e)}
                    </pre>
                </div>,
            );
        });
}

// Clickjacking (FRONTEND.md §9.3, M9-SEC-6): inside a frame the app is never mounted, whatever the
// host sends — the framing page gets a notice, not a wallet button or a Sign button.
if (isFramed()) root.render(<FrameGuard href={window.location.href} />);
else start();

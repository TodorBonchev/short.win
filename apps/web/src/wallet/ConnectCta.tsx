/**
 * "Connect wallet" where a form or page needs one (FRONTEND.md §5.4, §7.6): an enabled primary
 * button (`connect-cta-<surface>`) that opens the wallet chooser inline below it — the wallets, or
 * on a phone without one the open-in-wallet links. One chooser at a time (`walletUi.ts`): opening
 * this one closes the header's. The form's own submit stays rendered (disabled, with its reason).
 */
import { useEffect, useId } from 'react';
import { Button } from '../components/Button.tsx';
import { T } from '../config/testids.ts';
import { useStore } from '../lib/store.ts';
import { useWallet } from './useWallet.ts';
import { NoWallet, useWalletList, WalletOptions } from './WalletChooser.tsx';
import { type ConnectSurface, walletChooser } from './walletUi.ts';

export function ConnectCta({ surface, label = 'Connect wallet' }: { surface: ConnectSurface; label?: string }) {
    const { connected } = useWallet();
    const open = useStore(walletChooser) === `inline:${surface}`;
    const wallets = useWalletList();
    const panelId = useId();
    // Connected: the chooser has done its job.
    useEffect(() => {
        if (connected && open) walletChooser.set(null);
    }, [connected, open]);
    if (connected) return null;
    return (
        <div className="connect-cta">
            <Button
                variant="primary"
                block
                data-testid={T.connectCta(surface)}
                aria-expanded={open}
                aria-controls={panelId}
                onClick={() => walletChooser.set(open ? null : `inline:${surface}`)}
            >
                {label}
            </Button>
            {open && (
                <div id={panelId} className="wallet-inline">
                    {wallets.length === 0 ? (
                        <NoWallet />
                    ) : (
                        <WalletOptions wallets={wallets} asMenu={false} onPicked={() => walletChooser.set(null)} />
                    )}
                </div>
            )}
        </div>
    );
}

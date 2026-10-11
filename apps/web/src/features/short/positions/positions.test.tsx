/**
 * The positions dashboard and the position page's forms on program-derived snapshots
 * (`vectors/program-vectors.json`): the rows' live columns, the close preview equal to what
 * `close_short` paid, the add-margin CR after, the exit state in plain words, the live channels.
 */
import { type Address, createNoopSigner } from '@solana/kit';
import { fireEvent, render, screen } from '@testing-library/react';
import { beforeAll, describe, expect, it, vi } from 'vitest';
import type { Position } from '../../../api/types.ts';
import { ToastProvider } from '../../../components/Toast.tsx';
import { T } from '../../../config/testids.ts';
import type { MathModule } from '../../../math/load.ts';
import { positionMetrics } from '../previews.ts';
import { decodePosition } from '../snapshot.ts';
import { testMath, vectorCase, vectorSnapshot } from '../testing.ts';

const wallet = vi.hoisted(() => ({ address: '' }));

// The panel's Retry rebuilds trades through the app's RPC client (M9-UX-1); not exercised here.
vi.mock('../../../rpc/context.tsx', () => ({ useAppClient: () => ({ rpc: {}, rpcSubscriptions: {} }) }));
vi.mock('../../../wallet/useWallet.ts', () => ({
    useWallet: () => ({
        ready: true,
        status: 'connected',
        connected: wallet.address
            ? {
                  address: wallet.address,
                  signer: createNoopSigner(wallet.address as Address),
                  walletName: 'Test',
                  supportsV0: true,
              }
            : null,
    }),
}));
vi.mock('../hooks.ts', () => ({ useSolBalance: () => ({ data: 100_000_000_000n }) }));
vi.mock('../useTradeFlow.ts', () => ({
    useTradeFlow: () => ({ flow: idleFlow(), start: vi.fn(), building: false, buildError: null, fallback: null }),
}));
vi.mock('../../../tx/useTxFlow.ts', async (orig) => ({
    ...(await orig<typeof import('../../../tx/useTxFlow.ts')>()),
    useTxFlow: () => idleFlow(),
}));

function idleFlow() {
    return {
        state: {
            stage: 'idle',
            plan: null,
            report: null,
            acks: new Set(),
            speed: 'normal',
            signature: null,
            error: null,
            refreshed: false,
        },
        start: vi.fn(),
        refresh: vi.fn(),
        setAck: vi.fn(),
        setSpeed: vi.fn(),
        sign: vi.fn(),
        close: vi.fn(),
    };
}

const { CloseForm } = await import('./CloseForm.tsx');
const { AddMarginForm } = await import('./AddMarginForm.tsx');
const { exitShort, exitState, HealthBanner } = await import('./SettlementRisk.tsx');
const { buildRows } = await import('./data.ts');
const { liveChannels, positionTotals } = await import('../PositionsPage.tsx');

let math: MathModule;
beforeAll(() => {
    math = testMath();
});

async function closeCase(name: string) {
    const c = vectorCase(name);
    const { s, get } = await vectorSnapshot(c);
    const pos = decodePosition(get(c.position as Address));
    if (!pos) throw new Error('position');
    return { c, s, pos };
}

const v = (id: string) => screen.getByTestId(id).dataset.value;

describe('close form', () => {
    it('previews exactly what close_short paid', async () => {
        const { c, s, pos } = await closeCase('spl-close-after-fall');
        wallet.address = c.borrower;
        render(
            <ToastProvider>
                <CloseForm math={math} snapshot={s} position={c.position as Address} pos={pos} symbol="MOCK" />
            </ToastProvider>,
        );
        const e = c.result.event ?? {};
        expect(v(T.closePreview('buyback'))).toBe(String(e.buybackCost));
        expect(v(T.closePreview('rent'))).toBe(String(e.rent));
        expect(v(T.closePreview('payout'))).toBe(String(e.borrowerPayout));
        expect(BigInt(v(T.closePreview('pnl')) ?? 0)).toBe(BigInt(String(e.borrowerPayout)) - pos.m - pos.feeOpen);
        expect(screen.queryByTestId(T.closeStraddleNotice)).toBeNull();
        expect((screen.getByTestId(T.closeSubmit) as HTMLButtonElement).disabled).toBe(false);
        expect(screen.getByTestId(T.closeReason).textContent).toBe('');
    });

    it('is not offered to another wallet', async () => {
        const { c, s, pos } = await closeCase('spl-close-after-rise');
        wallet.address = 'BPFLoaderUpgradeab1e11111111111111111111111';
        render(
            <ToastProvider>
                <CloseForm math={math} snapshot={s} position={c.position as Address} pos={pos} symbol="MOCK" />
            </ToastProvider>,
        );
        expect((screen.getByTestId(T.closeSubmit) as HTMLButtonElement).disabled).toBe(true);
        expect(screen.getByTestId(T.closeReason).textContent).toBe('This position belongs to another wallet');
    });

    it('explains a vault that cannot pay the buyback', async () => {
        const { c, s, pos } = await closeCase('spl-close-after-rise');
        wallet.address = c.borrower;
        const thin = { ...pos, v: 1_000n };
        render(
            <ToastProvider>
                <CloseForm math={math} snapshot={s} position={c.position as Address} pos={thin} symbol="MOCK" />
            </ToastProvider>,
        );
        expect(screen.getByText(/Add margin to close/)).toBeTruthy();
        expect((screen.getByTestId(T.closeSubmit) as HTMLButtonElement).disabled).toBe(true);
        expect(screen.getByTestId(T.closeReason).textContent).toBe(
            'Add margin first: the vault cannot cover the buyback',
        );
    });
});

describe('add margin form', () => {
    it('shows the CR the position will have after add_margin', async () => {
        const c = vectorCase('spl-add-margin');
        const { s, get } = await vectorSnapshot(c);
        const pos = decodePosition(get(c.position as Address));
        if (!pos) throw new Error('position');
        wallet.address = c.borrower;
        render(
            <ToastProvider>
                <AddMarginForm math={math} snapshot={s} position={c.position as Address} pos={pos} symbol="MOCK" />
            </ToastProvider>,
        );
        fireEvent.change(screen.getByTestId(T.addMarginInput), { target: { value: '1.5' } });
        const after = c.result.position ?? {};
        const want = positionMetrics(math, s, { ...pos, v: BigInt(String(after.v)), m: BigInt(String(after.m)) }).crBps;
        expect(v(T.addMarginPreviewCr)).toBe(want?.toString());
        expect((screen.getByTestId(T.addMarginSubmit) as HTMLButtonElement).disabled).toBe(false);
        expect(screen.getByTestId(T.addMarginReason).textContent).toBe('');
        fireEvent.change(screen.getByTestId(T.addMarginInput), { target: { value: '100000' } });
        expect((screen.getByTestId(T.addMarginSubmit) as HTMLButtonElement).disabled).toBe(true);
        expect(screen.getByTestId(T.addMarginReason).textContent).toBe('Above your SOL balance less 0.01 SOL for fees');
        fireEvent.change(screen.getByTestId(T.addMarginInput), { target: { value: '' } });
        expect(screen.getByTestId(T.addMarginReason).textContent).toBe('Enter an amount of SOL');
    });
});

describe('exit state and the dashboard rows', () => {
    it('reads healthy, liquidatable, expired and armed states', async () => {
        const { s, pos } = await closeCase('spl-close-after-fall');
        const mx = positionMetrics(math, s, pos);
        expect(exitState(pos, mx, s.venue, s.now)).toBe('healthy');
        expect(exitState(pos, { ...mx, crBps: 1n, crEmaBps: 1n }, s.venue, s.now)).toBe('liquidatable');
        expect(exitState(pos, { ...mx, crBps: 1n }, s.venue, s.now)).toBe('spot-below');
        expect(exitState(pos, mx, s.venue, pos.deadline)).toBe('expired');
        expect(exitState({ ...pos, sArmed: 1 }, mx, s.venue, s.now)).toBe('armed');
        expect(exitState(pos, mx, { ...s.venue, executable: false }, s.now)).toBe('no-route');
    });

    it('past the deadline names the exit the program takes (expire.rs UseLiquidate, NoRoute; sol_settle)', async () => {
        const { s, pos } = await closeCase('spl-close-after-fall');
        const mx = positionMetrics(math, s, pos);
        const liq = BigInt(pos.crLiqBps);
        const at = pos.deadline;
        const crit = { ...mx, crBps: liq - 1n, crEmaBps: liq - 1n };
        // One second before the deadline: the CR states as ever.
        expect(exitState(pos, crit, s.venue, at - 1n)).toBe('liquidatable');
        expect(exitState(pos, mx, s.venue, at - 1n)).toBe('healthy');
        // At and after it, healthy: expire, with the expiry fee.
        expect(exitState(pos, mx, s.venue, at)).toBe('expired');
        expect(exitState(pos, mx, s.venue, at + 3_600n)).toBe('expired');
        // Both legs strictly below the line: expire refuses (UseLiquidate), liquidate takes it.
        expect(exitState(pos, crit, s.venue, at)).toBe('liquidatable');
        // Exactly at the line on both legs, or below on the live leg only: not CR-liquidatable.
        expect(exitState(pos, { ...mx, crBps: liq, crEmaBps: liq }, s.venue, at)).toBe('expired');
        expect(exitState(pos, { ...mx, crBps: liq - 1n, crEmaBps: liq * 2n }, s.venue, at)).toBe('expired');
        expect(exitState(pos, { ...mx, crBps: null }, s.venue, at)).toBe('expired');
        // No route: neither liquidate nor expire can run (NoRoute); S3 after the wait.
        const none = { ...s.venue, executable: false };
        expect(exitState(pos, mx, none, at)).toBe('no-route');
        expect(exitState(pos, crit, none, at)).toBe('no-route');
        expect(exitState({ ...pos, sArmed: 1 }, crit, none, at)).toBe('no-route');
        // Armed (S1 / S2 evidence) on an executable venue: the settlement clock keeps running.
        expect(exitState({ ...pos, sArmed: 1 }, mx, s.venue, at)).toBe('armed');
        expect(exitState({ ...pos, sArmed: 1 }, crit, s.venue, at)).toBe('armed');
        // Before the deadline the order is unchanged: armed before no route.
        expect(exitState({ ...pos, sArmed: 1 }, mx, none, at - 1n)).toBe('armed');
        expect(exitState(pos, crit, none, at - 1n)).toBe('no-route');
    });

    it('past the deadline the banner and the dashboard line read the state’s existing text', async () => {
        const { s, pos } = await closeCase('spl-close-after-fall');
        const mx = positionMetrics(math, s, pos);
        const liq = BigInt(pos.crLiqBps);
        const crit = { ...mx, crBps: liq - 1n, crEmaBps: liq - 1n };
        expect(exitShort(exitState(pos, crit, s.venue, pos.deadline), pos)).toBe('Liquidatable now');
        expect(exitShort(exitState(pos, mx, s.venue, pos.deadline), pos)).toBe('Past its deadline: closable by anyone');
        render(<HealthBanner pos={pos} metrics={crit} venue={s.venue} market={s.market} now={pos.deadline} />);
        const banner = document.querySelector('.position-health') as HTMLElement;
        expect(banner.dataset.state).toBe('liquidatable');
        expect(banner.textContent).toMatch(
            /^Liquidatable: your CR is below .* Anyone can liquidate this position now\.$/,
        );
    });

    it('builds live rows for open positions and exit rows for closed ones', async () => {
        const { c, s, pos } = await closeCase('spl-close-after-rise');
        const open: Position = {
            position: c.position as string,
            market: s.marketAddress,
            mint: s.mint,
            borrower: c.borrower,
            nonce: '0',
            status: 'open',
            tier: 'C',
            venue: 'mock',
            q: pos.q.toString(),
            s: pos.s.toString(),
            m: pos.m.toString(),
            margin_added: '0',
            v: pos.v.toString(),
            n_open: pos.nOpen.toString(),
            fee_open: pos.feeOpen.toString(),
            entry_price: pos.entryPrice.toString(),
            deadline: Number(pos.deadline),
            opened_ts: Number(pos.tOpen),
            open_signature: '1111111111111111111111111111111111111111111111111111111111111111',
            armed: false,
            exit: null,
        };
        const closed: Position = {
            ...open,
            position: 'So11111111111111111111111111111111111111112',
            status: 'liquidated',
        };
        const rows = buildRows({
            math,
            usingChain: false,
            api: [open, closed],
            chainOpen: [],
            accounts: new Map([[c.position as Address, pos]]),
            // The live columns are priced on the mint of the position's own market, from the chain (M9-SEC-2).
            marketMints: new Map([[s.marketAddress, s.mint]]),
            snapshots: new Map([[s.mint, s]]),
            status: 'all',
        });
        expect(rows).toHaveLength(2);
        expect(rows[0]?.mismatch).toBe(false);
        expect(rows[0]?.metrics?.payout).toBe(BigInt(String(c.result.event?.borrowerPayout)));
        expect(rows[1]?.metrics).toBeNull();
        expect(liveChannels(rows)).toEqual([`position:${c.position}`]);
        // The totals line: the open vault and rent due; P&L of the rows that have one (the open row).
        const t = positionTotals(rows);
        expect(t).toMatchObject({ count: 2, open: 1, vault: pos.v, missingPnl: 1 });
        expect(t.pnl).toBe(rows[0]?.metrics?.pnl);
        expect(t.rent).toBe(rows[0]?.metrics?.rentDue);
        const many = Array.from({ length: 50 }, (_, i) => ({ ...rows[0], position: `p${i}` }) as (typeof rows)[number]);
        expect(liveChannels(many)).toHaveLength(40);
    });
});

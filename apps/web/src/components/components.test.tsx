import { fireEvent, render, screen } from '@testing-library/react';
import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { LENDER_SOL_REPAYMENT } from '../config/disclosures.ts';
import { T } from '../config/testids.ts';
import { RiskBadge, riskLevel, TierBadge } from './Badge.tsx';
import { DataTable } from './DataTable.tsx';
import { Disclosure } from './Disclosure.tsx';
import { AmountInput } from './Field.tsx';
import { Pnl, Price, Sol, Tokens } from './Num.tsx';

describe('number cells', () => {
    it('render raw values in data-value and signed P&L with sign, arrow and words', () => {
        render(
            <>
                <Sol lamports={1_234_500_000n} testId="sol" />
                <Pnl lamports={-450_000_000n} testId="pnl" />
                <Price wad={28_000_000_000_000n} decimals={6} testId="price" />
            </>,
        );
        expect(screen.getByTestId('sol').dataset.value).toBe('1234500000');
        expect(screen.getByTestId('sol').textContent).toBe('1.2345SOL');
        const pnl = screen.getByTestId('pnl');
        expect(pnl.dataset.sign).toBe('down');
        expect(pnl.textContent).toContain('▼');
        expect(pnl.textContent).toContain('loss 0.45');
        expect(screen.getByTestId('price').textContent).toContain('0.000000028 SOL per token');
    });

    it('mute an amount by its displayed digits: dust that rounds to zero is as quiet as an exact zero', () => {
        render(
            <>
                <Sol lamports={0n} testId="zero" />
                <Sol lamports={1n} testId="dust" />
                <Sol lamports={1n} frac={9} testId="dust-shown" />
                <Sol lamports={1_000_000n} pad testId="dust-pad" />
                <Tokens raw={1n} decimals={6} frac={4} pad testId="dust-tokens" />
            </>,
        );
        expect(screen.getByTestId('zero').dataset.zero).toBe('');
        // The raw value stays on the cell; only the quiet colour follows the digits shown.
        expect(screen.getByTestId('dust').dataset.zero).toBe('');
        expect(screen.getByTestId('dust').dataset.value).toBe('1');
        expect(screen.getByTestId('dust-shown').dataset.zero).toBeUndefined();
        expect(screen.getByTestId('dust-pad').textContent).toBe('0.0010SOL');
        expect(screen.getByTestId('dust-pad').dataset.zero).toBeUndefined();
        expect(screen.getByTestId('dust-tokens').textContent).toBe('0.0000');
        expect(screen.getByTestId('dust-tokens').dataset.zero).toBe('');
    });
});

describe('badges', () => {
    it('classify risk from CR against the liquidation CR, with a word', () => {
        // Liquidatable only strictly below on both legs, as the program (M9-UX-4).
        expect(riskLevel(12_499n, 12_499n, 12_500)).toBe('critical');
        expect(riskLevel(12_499n, 12_500n, 12_500)).toBe('below');
        expect(riskLevel(12_500n, 12_000n, 12_500)).toBe('danger');
        expect(riskLevel(13_000n, null, 12_500)).toBe('danger');
        expect(riskLevel(15_000n, null, 12_500)).toBe('watch');
        expect(riskLevel(20_000n, null, 12_500)).toBe('safe');
        render(
            <>
                <RiskBadge level="critical" testId="risk" />
                <TierBadge tier="D" testId="tier" />
            </>,
        );
        expect(screen.getByTestId('risk').textContent).toBe('Liquidatable');
        expect(screen.getByTestId('tier').textContent).toBe('Tier D');
    });
});

describe('Disclosure', () => {
    it('renders the SPEC §5.3 sentence verbatim with its required checkbox', () => {
        const onAck = vi.fn();
        render(<Disclosure id="lender.solRepayment" acknowledged={false} onAcknowledge={onAck} />);
        const box = screen.getByTestId(T.disclosure('lender.solRepayment'));
        expect(box.textContent).toContain(LENDER_SOL_REPAYMENT);
        // Once, as the checkbox's label (the acknowledgement is the sentence itself).
        expect(box.textContent?.split(LENDER_SOL_REPAYMENT).length).toBe(2);
        expect(screen.getByTestId(T.disclosureAck('lender.solRepayment')).closest('label')?.textContent).toBe(
            LENDER_SOL_REPAYMENT,
        );
        fireEvent.click(screen.getByTestId(T.disclosureAck('lender.solRepayment')));
        expect(onAck).toHaveBeenCalledWith(true);
    });
});

describe('DataTable', () => {
    it('sorts through header buttons with aria-sort and activates rows with Enter', () => {
        const onSort = vi.fn();
        const onRow = vi.fn();
        render(
            <DataTable
                caption="Rows"
                columns={[
                    { key: 'name', header: 'Name', render: (r: { id: string }) => r.id },
                    {
                        key: 'tvl',
                        header: 'TVL',
                        sortable: true,
                        align: 'right',
                        render: () => '1',
                        testId: 'sort-tvl',
                    },
                ]}
                rows={[{ id: 'a' }, { id: 'b' }]}
                rowKey={(r) => r.id}
                rowTestId={(r) => `row-${r.id}`}
                sort={{ key: 'tvl', order: 'desc' }}
                onSort={onSort}
                onRowActivate={onRow}
            />,
        );
        expect(screen.getByRole('columnheader', { name: /TVL/ }).getAttribute('aria-sort')).toBe('descending');
        fireEvent.click(screen.getByTestId('sort-tvl'));
        expect(onSort).toHaveBeenCalledWith('tvl');
        fireEvent.keyDown(screen.getByTestId('row-b'), { key: 'Enter' });
        expect(onRow).toHaveBeenCalledWith({ id: 'b' });
    });

    it('marks the sorted column on data rows only: a loading skeleton row carries no data-sorted', () => {
        const { container } = render(
            <DataTable
                caption="Rows"
                columns={[
                    { key: 'name', header: 'Name', render: (r: { id: string }) => r.id },
                    { key: 'tvl', header: 'TVL', sortable: true, align: 'right', render: () => '1' },
                ]}
                rows={undefined}
                loading
                rowKey={(r) => r.id}
                sort={{ key: 'tvl', order: 'desc' }}
                onSort={() => undefined}
            />,
        );
        expect(container.querySelectorAll('tbody .skeleton').length).toBeGreaterThan(0);
        expect(container.querySelectorAll('tbody td[data-col="tvl"]').length).toBeGreaterThan(0);
        expect(container.querySelectorAll('tbody td[data-sorted]').length).toBe(0);
        expect(container.querySelector('th[aria-sort="descending"]')).not.toBeNull();
    });
});

describe('AmountInput', () => {
    function Harness() {
        const [v, setV] = useState('');
        return (
            <AmountInput
                label="Amount"
                value={v}
                onChange={(t) => setV(t)}
                decimals={2}
                unit="BONK"
                max={12_345n}
                formatMax={(raw) => (Number(raw) / 100).toString()}
                testId="amt"
                maxTestId="amt-max"
            />
        );
    }

    it('validates strictly and fills Max', () => {
        render(<Harness />);
        const input = screen.getByTestId('amt') as HTMLInputElement;
        fireEvent.change(input, { target: { value: '1.234' } });
        expect(screen.getByRole('alert').textContent).toBe('Too many decimal places for this token.');
        expect(input.getAttribute('aria-invalid')).toBe('true');
        fireEvent.change(input, { target: { value: '1,5' } });
        expect(input.value).toBe('1.5');
        expect(screen.queryByRole('alert')).toBeNull();
        fireEvent.click(screen.getByTestId('amt-max'));
        expect(input.value).toBe('123.45');
    });
});

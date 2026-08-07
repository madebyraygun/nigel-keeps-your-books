import { describe, it, expect } from 'vitest';
import { cashflowBuckets } from './dashboard-data.js';
import type { CashflowMonth, CashflowReport } from '../api/types.js';

function month(m: string, inflows = 1000, outflows = -400): CashflowMonth {
  return { month: m, inflows, outflows, net: inflows + outflows, runningBalance: 0 };
}

function report(months: CashflowMonth[]): CashflowReport {
  return { months };
}

describe('cashflowBuckets', () => {
  it('keeps only the last twelve months', () => {
    const months = Array.from({ length: 14 }, (_, i) =>
      month(`2025-${String(i + 1).padStart(2, '0')}`),
    );
    const { buckets } = cashflowBuckets(report(months));

    expect(buckets.length).toBe(12);
    // The first two of fourteen fall off the front.
    expect(buckets[0]?.label).toBe('Mar');
  });

  it('does not pad a short history out to twelve', () => {
    const { buckets } = cashflowBuckets(
      report([month('2026-01'), month('2026-02'), month('2026-03')]),
    );
    expect(buckets.map((b) => b.label)).toEqual(['Jan', 'Feb', 'Mar']);
  });

  it('leaves gaps alone rather than zero-filling them', () => {
    // The server emits only months with transactions; the TUI charts exactly
    // these, so the web must not invent the missing ones.
    const { buckets } = cashflowBuckets(
      report([month('2026-01'), month('2026-06')]),
    );
    expect(buckets.map((b) => b.label)).toEqual(['Jan', 'Jun']);
  });

  it('turns negative outflows into positive expense magnitudes', () => {
    const { buckets } = cashflowBuckets(report([month('2026-01', 5000, -3200)]));
    expect(buckets[0]).toEqual({ label: 'Jan', income: 5000, expense: 3200 });
  });

  it('keeps the cents the TUI truncates', () => {
    const { buckets } = cashflowBuckets(
      report([month('2026-01', 5000.55, -3200.49)]),
    );
    expect(buckets[0]?.income).toBe(5000.55);
    expect(buckets[0]?.expense).toBe(3200.49);
  });

  it('floors a negative inflow at zero rather than drawing it downward', () => {
    const { buckets } = cashflowBuckets(report([month('2026-01', -5, -100)]));
    expect(buckets[0]?.income).toBe(0);
  });

  it('captions a window inside one year with that year', () => {
    const { caption } = cashflowBuckets(
      report([month('2025-01'), month('2025-12')]),
    );
    expect(caption).toBe('2025');
  });

  it('abbreviates the second year when the window crosses a boundary', () => {
    const { caption } = cashflowBuckets(
      report([month('2025-03'), month('2026-02')]),
    );
    expect(caption).toBe('2025 - 26');
  });

  it('captions from the windowed months, not the whole report', () => {
    const months = [
      month('2023-01'),
      ...Array.from({ length: 12 }, (_, i) =>
        month(`2026-${String(i + 1).padStart(2, '0')}`),
      ),
    ];
    expect(cashflowBuckets(report(months)).caption).toBe('2026');
  });

  it('has nothing to draw or caption for an empty report', () => {
    expect(cashflowBuckets(report([]))).toEqual({ buckets: [], caption: '' });
  });

  it('passes an unparseable month through as its own label', () => {
    const { buckets } = cashflowBuckets(report([month('2026')]));
    expect(buckets[0]?.label).toBe('2026');
  });

  it('names every month correctly', () => {
    const months = Array.from({ length: 12 }, (_, i) =>
      month(`2026-${String(i + 1).padStart(2, '0')}`),
    );
    expect(cashflowBuckets(report(months)).buckets.map((b) => b.label)).toEqual([
      'Jan',
      'Feb',
      'Mar',
      'Apr',
      'May',
      'Jun',
      'Jul',
      'Aug',
      'Sep',
      'Oct',
      'Nov',
      'Dec',
    ]);
  });
});

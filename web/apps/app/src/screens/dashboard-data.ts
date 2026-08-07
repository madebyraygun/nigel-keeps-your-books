import type { BarBucket } from '@nigel/ui';
import type { CashflowReport } from '../api/types.js';

/** What the chart needs, derived from a cash flow report. */
export interface CashflowChart {
  buckets: BarBucket[];
  /** The period the buckets span, e.g. "2025" or "2025 - 26". */
  caption: string;
}

const MONTH_NAMES = [
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
];

/** `2026-03` becomes `Mar`; anything else is passed through unchanged. */
function monthLabel(month: string): string {
  const parts = month.split('-');
  if (parts.length !== 2) return month;
  const index = Number(parts[1]);
  return MONTH_NAMES[index - 1] ?? month;
}

/**
 * The period the buckets cover, in the TUI's short form.
 *
 * A window that stays inside one year is just that year; one that crosses a
 * boundary abbreviates the second, so twelve months ending in early 2026 read
 * as "2025 - 26" rather than the wider "2025 - 2026".
 */
function caption(first: string, last: string): string {
  const firstYear = first.slice(0, 4);
  const lastYear = last.slice(0, 4);
  return firstYear === lastYear
    ? firstYear
    : `${firstYear} - ${lastYear.slice(2)}`;
}

/**
 * The last twelve months of cash flow, as chart buckets.
 *
 * A tail slice of the months that *have data* rather than twelve calendar
 * months: `get_cashflow` emits only months with transactions, and the TUI
 * charts exactly these. Zero-filling the gaps here would make the two front
 * ends disagree about the same books, which is a worse fault than a chart whose
 * twelve bars occasionally span more than twelve months.
 *
 * Outflows arrive negative — they are the sum of the negative amounts — and
 * become positive magnitudes, because the chart draws bar heights and a sign
 * has nowhere to go in one. Unlike the TUI, which truncates to whole dollars
 * for its terminal renderer, the cents are kept.
 */
export function cashflowBuckets(report: CashflowReport): CashflowChart {
  const recent = report.months.slice(-12);

  const buckets: BarBucket[] = recent.map((m) => ({
    label: monthLabel(m.month),
    income: Math.max(m.inflows, 0),
    expense: Math.abs(m.outflows),
  }));

  const first = recent[0];
  const last = recent[recent.length - 1];

  return {
    buckets,
    caption: first && last ? caption(first.month, last.month) : '',
  };
}

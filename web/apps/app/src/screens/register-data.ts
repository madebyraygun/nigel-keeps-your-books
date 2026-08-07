import { paramsToPeriod, periodToParams, type NcEditCommitDetail } from '@nigel/ui';
import type { RegisterParams } from '../api/client.js';
import type { RegisterRow, TransactionPatch } from '../api/types.js';

const ISO_DATE = /^\d{4}-\d{2}-\d{2}$/;

/**
 * The TUI's `recompute_search_matches`, field for field.
 *
 * Description, vendor and category name, case-insensitive substring, with a
 * missing vendor or category treated as an empty string so it can never match.
 * Date, amount, id and account are deliberately not searched — widening the
 * predicate here would quietly make the web and the terminal disagree about
 * what a search means.
 *
 * The query is not trimmed, for the same reason: in the TUI a trailing space
 * is part of what you typed, and "adobe c" matching "ADOBE CREATIVE CLOUD"
 * depends on it.
 */
export function rowMatches(row: RegisterRow, query: string): boolean {
  if (query === '') return true;
  const needle = query.toLowerCase();
  return (
    row.description.toLowerCase().includes(needle) ||
    (row.vendor ?? '').toLowerCase().includes(needle) ||
    (row.category ?? '').toLowerCase().includes(needle)
  );
}

export function filterRows(rows: RegisterRow[], query: string): RegisterRow[] {
  if (query === '') return rows;
  return rows.filter((row) => rowMatches(row, query));
}

/**
 * Today as `YYYY-MM-DD` in the local timezone.
 *
 * Not `toISOString()`, which is UTC: west of Greenwich it reports tomorrow for
 * most of the evening, and the register would open on the wrong row.
 */
export function todayIso(now: Date = new Date()): string {
  const pad = (value: number) => String(value).padStart(2, '0');
  return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
}

/**
 * The row `scroll_to_today` lands on: the last one dated on or before today,
 * relying on the date-ascending order the register endpoint returns.
 *
 * Returns -1 when every transaction is in the future, which the TUI answers by
 * staying at the top of the list.
 */
export function indexOfToday(rows: RegisterRow[], today: string): number {
  for (let index = rows.length - 1; index >= 0; index -= 1) {
    const row = rows[index];
    if (row && row.date <= today) return index;
  }
  return -1;
}

/**
 * The API request a set of route parameters describes.
 *
 * `from`/`to` are honoured only as a pair, because one without the other is a
 * 400, and they win over `year`/`month` rather than being sent alongside them.
 */
export function registerParamsFrom(params: URLSearchParams): RegisterParams {
  const request: RegisterParams = {};

  const from = params.get('from');
  const to = params.get('to');
  if (from && to && ISO_DATE.test(from) && ISO_DATE.test(to)) {
    request.from = from;
    request.to = to;
  } else {
    Object.assign(request, periodToParams(paramsToPeriod(params)));
  }

  const account = params.get('account');
  if (account) request.account = account;

  return request;
}

/**
 * The smallest legal patch for an edit, or null when nothing changed.
 *
 * A `PATCH` with no recognized field is a 400 by design, so an unchanged
 * commit must not be sent at all. `categoryId` is omitted when the editor
 * reports none: `null` there is also a 400, since uncategorizing is what the
 * review undo route is for.
 */
export function buildPatch(
  row: RegisterRow,
  detail: NcEditCommitDetail,
): TransactionPatch | null {
  const patch: TransactionPatch = {};

  if (detail.categoryId !== null && detail.categoryId !== row.categoryId) {
    patch.categoryId = detail.categoryId;
  }
  if (detail.vendor !== (row.vendor ?? null)) {
    patch.vendor = detail.vendor;
  }

  return Object.keys(patch).length === 0 ? null : patch;
}

/** Replace one row in place, keeping every other row identical. */
export function replaceRow(rows: RegisterRow[], updated: RegisterRow): RegisterRow[] {
  return rows.map((row) => (row.id === updated.id ? updated : row));
}

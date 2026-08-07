import { describe, it, expect } from 'vitest';
import {
  buildPatch,
  filterRows,
  indexOfToday,
  registerParamsFrom,
  replaceRow,
  rowMatches,
  todayIso,
} from './register-data.js';
import type { RegisterRow } from '../api/types.js';

function row(overrides: Partial<RegisterRow> = {}): RegisterRow {
  return {
    id: 1,
    date: '2025-03-04',
    description: 'ADOBE CREATIVE CLOUD',
    amount: -54.99,
    category: 'Software / Subscriptions',
    categoryId: 12,
    vendor: 'Adobe Inc',
    accountName: 'BofA Credit Card',
    isFlagged: false,
    ...overrides,
  };
}

/**
 * The predicate the TUI implements in `recompute_search_matches`. Each case
 * here is a promise about what a search means, in either half of the app.
 */
describe('rowMatches', () => {
  it('matches an uppercase description from a lowercase query', () => {
    expect(rowMatches(row(), 'adobe')).toBe(true);
  });

  it('matches the vendor', () => {
    expect(rowMatches(row({ description: 'SQ *PURCHASE' }), 'adobe inc')).toBe(true);
  });

  it('matches the category name', () => {
    expect(rowMatches(row({ description: 'SQ *PURCHASE', vendor: null }), 'software')).toBe(
      true,
    );
  });

  it('matches a substring, not just a prefix', () => {
    expect(rowMatches(row(), 'creative')).toBe(true);
  });

  it('matches across a description, spaces and all', () => {
    expect(rowMatches(row(), 'adobe c')).toBe(true);
  });

  it('never matches a missing vendor or category', () => {
    const bare = row({ description: 'SQ *PURCHASE', vendor: null, category: null });
    expect(rowMatches(bare, 'adobe')).toBe(false);
    expect(rowMatches(bare, '—')).toBe(false);
  });

  it('does not search the date, the amount or the account', () => {
    const bare = row({ description: 'SQ *PURCHASE', vendor: null, category: null });
    expect(rowMatches(bare, '2025-03-04')).toBe(false);
    expect(rowMatches(bare, '54.99')).toBe(false);
    expect(rowMatches(bare, 'BofA')).toBe(false);
  });

  it('matches everything on an empty query', () => {
    expect(rowMatches(row(), '')).toBe(true);
  });
});

describe('filterRows', () => {
  const rows = [
    row({ id: 1 }),
    row({ id: 2, description: 'CLIENT PAYMENT', vendor: null, category: 'Consulting' }),
    row({ id: 3, description: 'BANK FEE', vendor: null, category: null }),
  ];

  it('keeps only matching rows, in the order they came', () => {
    expect(filterRows(rows, 'a').map((r) => r.id)).toEqual([1, 2, 3]);
    expect(filterRows(rows, 'consult').map((r) => r.id)).toEqual([2]);
    expect(filterRows(rows, 'zzz')).toEqual([]);
  });

  it('returns the same array when nothing is searched for', () => {
    expect(filterRows(rows, '')).toBe(rows);
  });
});

describe('todayIso', () => {
  it('formats the local date, zero padded', () => {
    expect(todayIso(new Date(2025, 0, 5, 13, 45))).toBe('2025-01-05');
  });

  it('reports the local day even when UTC has already moved on', () => {
    // 23:30 on the 5th, local. toISOString() would say the 6th east of UTC.
    expect(todayIso(new Date(2025, 2, 5, 23, 30))).toBe('2025-03-05');
  });
});

describe('indexOfToday', () => {
  const rows = [
    row({ id: 1, date: '2025-03-01' }),
    row({ id: 2, date: '2025-03-04' }),
    row({ id: 3, date: '2025-03-04' }),
    row({ id: 4, date: '2025-03-09' }),
  ];

  it('lands on the last row dated today', () => {
    expect(indexOfToday(rows, '2025-03-04')).toBe(2);
  });

  it('lands on the nearest earlier row when today has no transaction', () => {
    expect(indexOfToday(rows, '2025-03-06')).toBe(2);
  });

  it('lands on the final row when every transaction is in the past', () => {
    expect(indexOfToday(rows, '2026-01-01')).toBe(3);
  });

  it('reports nothing when every transaction is in the future', () => {
    expect(indexOfToday(rows, '2024-01-01')).toBe(-1);
  });

  it('reports nothing for an empty register', () => {
    expect(indexOfToday([], '2025-03-04')).toBe(-1);
  });
});

describe('registerParamsFrom', () => {
  it('asks for everything when the route carries no filters', () => {
    expect(registerParamsFrom(new URLSearchParams())).toEqual({});
  });

  it('carries the account by name', () => {
    expect(registerParamsFrom(new URLSearchParams('account=BofA Checking'))).toEqual({
      account: 'BofA Checking',
    });
  });

  it('carries a year and a month in the wire shapes', () => {
    expect(registerParamsFrom(new URLSearchParams('year=2025'))).toEqual({ year: 2025 });
    expect(registerParamsFrom(new URLSearchParams('month=2025-03'))).toEqual({
      month: '2025-03',
    });
  });

  it('takes from and to only as a pair', () => {
    expect(
      registerParamsFrom(new URLSearchParams('from=2025-01-01&to=2025-02-28')),
    ).toEqual({ from: '2025-01-01', to: '2025-02-28' });
    expect(registerParamsFrom(new URLSearchParams('from=2025-01-01'))).toEqual({});
  });

  it('lets an explicit range win rather than sending both axes', () => {
    expect(
      registerParamsFrom(new URLSearchParams('year=2025&from=2025-01-01&to=2025-02-28')),
    ).toEqual({ from: '2025-01-01', to: '2025-02-28' });
  });

  it('ignores parameters that are not filters', () => {
    expect(registerParamsFrom(new URLSearchParams('q=adobe&id=185'))).toEqual({});
  });
});

describe('buildPatch', () => {
  it('sends only what changed', () => {
    expect(buildPatch(row(), { id: 1, categoryId: 21, vendor: 'Adobe Inc' })).toEqual({
      categoryId: 21,
    });
    expect(buildPatch(row(), { id: 1, categoryId: 12, vendor: 'Adobe' })).toEqual({
      vendor: 'Adobe',
    });
  });

  it('sends both when both changed', () => {
    expect(buildPatch(row(), { id: 1, categoryId: 21, vendor: 'Adobe' })).toEqual({
      categoryId: 21,
      vendor: 'Adobe',
    });
  });

  it('clears a vendor with null', () => {
    expect(buildPatch(row(), { id: 1, categoryId: 12, vendor: null })).toEqual({
      vendor: null,
    });
  });

  it('sends nothing at all when nothing changed', () => {
    expect(buildPatch(row(), { id: 1, categoryId: 12, vendor: 'Adobe Inc' })).toBeNull();
  });

  it('never sends a null category, which the API refuses', () => {
    const uncategorized = row({ category: null, categoryId: null });
    expect(buildPatch(uncategorized, { id: 1, categoryId: null, vendor: 'Adobe Inc' })).toBeNull();
  });
});

describe('replaceRow', () => {
  it('swaps one row and leaves the others identical', () => {
    const rows = [row({ id: 1 }), row({ id: 2 })];
    const updated = { ...row({ id: 2 }), vendor: 'Someone else' };
    const next = replaceRow(rows, updated);

    expect(next[1]).toBe(updated);
    expect(next[0]).toBe(rows[0]);
  });
});

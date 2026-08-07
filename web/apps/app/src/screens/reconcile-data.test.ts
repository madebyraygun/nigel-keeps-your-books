import { describe, it, expect } from 'vitest';

import { initialAccount, reconcileFailure, toHistoryRows } from './reconcile-data.js';
import { ApiError } from '../api/index.js';
import { conflictError } from '../__mocks__/fake-api-client.js';
import type { ReconciliationRecord } from '../api/types.js';

const RECORDS: ReconciliationRecord[] = [
  {
    id: 4,
    accountId: 1,
    accountName: 'BofA Checking',
    month: '2025-03',
    statementBalance: 5000,
    calculatedBalance: 4871.44,
    isReconciled: false,
    reconciledAt: null,
    notes: null,
  },
  {
    id: 1,
    accountId: 1,
    accountName: 'BofA Checking',
    month: '2024-12',
    statementBalance: null,
    calculatedBalance: null,
    isReconciled: true,
    reconciledAt: '2025-01-04 10:00:00',
    notes: 'imported from the old books',
  },
];

describe('toHistoryRows', () => {
  it('keeps the server’s newest-first order', () => {
    expect(toHistoryRows(RECORDS).map((row) => row.month)).toEqual([
      '2025-03',
      '2024-12',
    ]);
  });

  it('passes a null balance through instead of defaulting it to zero', () => {
    const [, older] = toHistoryRows(RECORDS);
    expect(older.statementBalance).toBeNull();
    expect(older.calculatedBalance).toBeNull();
  });

  it('drops the fields the table does not show', () => {
    expect(Object.keys(toHistoryRows(RECORDS)[0]).sort()).toEqual([
      'calculatedBalance',
      'id',
      'isReconciled',
      'month',
      'reconciledAt',
      'statementBalance',
    ]);
  });
});

describe('reconcileFailure', () => {
  it('puts an empty month under the month field, in our own words', () => {
    const failure = reconcileFailure(
      conflictError('no_transactions', {
        message: 'No transactions for BofA Checking in 2025-07',
        account: 'BofA Checking',
        month: '2025-07',
      }),
    );

    expect(failure.field).toBe('month');
    expect(failure.message).toBe('No transactions for that account in that month.');
  });

  it('puts an unknown account under the account field, in words a browser can act on', () => {
    // The server's sentence says to run `nigel accounts list`, which is good
    // advice in a terminal and useless beside an account picker.
    const failure = reconcileFailure(
      new ApiError({
        code: 'not_found',
        rawCode: 'not_found',
        message:
          "Account 'Nope' not found. Run `nigel accounts list` to see available accounts, or `nigel accounts add` to create one.",
        status: 404,
      }),
    );

    expect(failure.field).toBe('account');
    expect(failure.message).toBe(
      'That account no longer exists. Reload to see the current list.',
    );
  });

  it('leaves an unplaceable failure above the form with the server’s sentence', () => {
    const failure = reconcileFailure(
      new ApiError({
        code: 'bad_request',
        rawCode: 'bad_request',
        message: 'Invalid month: 2025-13',
        status: 400,
      }),
    );

    expect(failure.field).toBeUndefined();
    expect(failure.message).toBe('Invalid month: 2025-13');
  });

  it('falls back to its own words when the failure is not an ApiError', () => {
    const failure = reconcileFailure(new TypeError('boom'));
    expect(failure.field).toBeUndefined();
    expect(failure.message).toBe('Could not reconcile that month.');
  });

  it('does not mistake another 409 for an empty month', () => {
    const failure = reconcileFailure(conflictError('duplicate_name', { name: 'x' }));
    expect(failure.field).toBeUndefined();
  });
});

describe('initialAccount', () => {
  const accounts = ['BofA Checking', 'BofA Credit Card'];

  it('opens on the deep-linked account', () => {
    expect(
      initialAccount(new URLSearchParams('account=BofA Credit Card'), accounts),
    ).toBe('BofA Credit Card');
  });

  it('falls back to the first, where the TUI’s selector starts', () => {
    expect(initialAccount(new URLSearchParams(), accounts)).toBe('BofA Checking');
    expect(initialAccount(new URLSearchParams('account=Gone'), accounts)).toBe(
      'BofA Checking',
    );
  });

  it('has nothing to choose from an empty list', () => {
    expect(initialAccount(new URLSearchParams('account=BofA Checking'), [])).toBe('');
  });
});

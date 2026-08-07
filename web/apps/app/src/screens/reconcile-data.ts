import type { ReconciliationHistoryRow } from '@nigel/ui';

import { ApiError } from '../api/index.js';
import type { ReconciliationRecord } from '../api/types.js';
import { conflictDetailsOf } from './manager-errors.js';

/**
 * The history rows, as the component wants them.
 *
 * No sorting: `list_reconciliations` orders by month descending and the
 * screen shows what the server decided. Re-sorting here would be a second
 * opinion that eventually disagrees with the one the API documents.
 */
export function toHistoryRows(
  records: ReconciliationRecord[],
): ReconciliationHistoryRow[] {
  return records.map((record) => ({
    id: record.id,
    month: record.month,
    statementBalance: record.statementBalance,
    calculatedBalance: record.calculatedBalance,
    isReconciled: record.isReconciled,
    reconciledAt: record.reconciledAt,
  }));
}

/** Which control a failed reconcile belongs under, and what it should say. */
export interface ReconcileFailure {
  field?: 'account' | 'month';
  message: string;
}

/**
 * Put a rejected reconcile next to the input that caused it.
 *
 * The two the route can raise are worth naming: a 409 `no_transactions` is
 * about the month, and a 404 is about the account. Both are answered in our
 * own words, which `docs/api.md` says is the point of the reason codes.
 * Anything else keeps the server's sentence and sits above the form, since a
 * message we cannot place is worse under a field than beside the whole thing.
 *
 * The 404 is worded here rather than passed through for a specific reason: the
 * server's sentence tells you to run `nigel accounts list`, which is good
 * advice in a terminal and useless in a browser that is already showing the
 * account picker. Reaching it at all means the list went stale under the form.
 */
export function reconcileFailure(error: unknown): ReconcileFailure {
  if (conflictDetailsOf(error)?.reason === 'no_transactions') {
    return {
      field: 'month',
      message: 'No transactions for that account in that month.',
    };
  }

  if (error instanceof ApiError && error.status === 404) {
    return {
      field: 'account',
      message: 'That account no longer exists. Reload to see the current list.',
    };
  }

  if (error instanceof ApiError) return { message: error.message };
  return { message: 'Could not reconcile that month.' };
}

/**
 * Which account the screen opens on.
 *
 * `#/reconcile?account=BofA%20Checking` wins when it names one that exists;
 * otherwise the first, which is where `reconcile_manager.rs` starts its
 * selector. An empty list has nothing to choose.
 */
export function initialAccount(params: URLSearchParams, accounts: string[]): string {
  const requested = params.get('account');
  if (requested !== null && accounts.includes(requested)) return requested;
  return accounts[0] ?? '';
}

import { ApiError } from '../api/index.js';
import type { FlaggedTxn, NotFoundDetails, RegisterRow } from '../api/types.js';

/**
 * One transaction as the review screen holds it.
 *
 * The queue answers with `FlaggedTxn` and a re-review by id answers with a
 * full `RegisterRow`; normalizing both to this shape on the way in means the
 * card and the form never ask which mode they are in.
 */
export interface ReviewItem {
  id: number;
  date: string;
  description: string;
  amount: number;
  accountName: string;
  category: string | null;
  vendor: string | null;
}

/**
 * What one step of the review did, kept so Back can take it apart again.
 *
 * `null` is a skip — the same `Option<ReviewDecision>` the TUI pushes onto its
 * stack, and the reason stepping back over a skipped transaction issues no
 * undo: there is nothing to undo, and calling undo anyway would clear a
 * category some earlier session had set.
 */
export type Decision = { transactionId: number; ruleId: number | null } | null;

export interface ReviewSummary {
  reviewed: number;
  skipped: number;
  rulesCreated: number;
}

export function toReviewItem(txn: FlaggedTxn | RegisterRow): ReviewItem {
  return {
    id: txn.id,
    date: txn.date,
    description: txn.description,
    amount: txn.amount,
    accountName: txn.accountName,
    category: 'category' in txn ? txn.category : null,
    vendor: 'vendor' in txn ? txn.vendor : null,
  };
}

/**
 * The counts the completion summary shows.
 *
 * Derived from the decision stack rather than tallied as we go, so a Back that
 * pops a decision corrects the totals for free — a separate counter would have
 * to remember to decrement, and one day would not.
 */
export function summarize(history: Decision[]): ReviewSummary {
  let reviewed = 0;
  let skipped = 0;
  let rulesCreated = 0;

  for (const decision of history) {
    if (decision === null) {
      skipped += 1;
      continue;
    }
    reviewed += 1;
    if (decision.ruleId !== null) rulesCreated += 1;
  }

  return { reviewed, skipped, rulesCreated };
}

/**
 * Whether a failed apply means the transaction itself has gone.
 *
 * `POST /api/review/:id/apply` answers 404 for two unrelated things, and they
 * call for opposite handling: a transaction another tab has taken away is one
 * to skip past, while a category that has been deleted leaves a decision still
 * to be made on a transaction that is still there. The reason code is what
 * tells them apart, so a 404 without one is treated as the second — the queue
 * survives being wrong about that, the books do not survive the reverse.
 */
export function isMissingTransaction(error: unknown): boolean {
  if (!(error instanceof ApiError) || error.status !== 404) return false;
  const details = error.details as NotFoundDetails | undefined;
  return details?.reason === 'transaction_not_found';
}

/**
 * The transaction id a single re-review was asked for, or null for the queue.
 *
 * `#/review?id=185`, matching `nigel review --id 185`. The router has no path
 * segments, so the id is a query parameter like every other deep link.
 */
export function singleIdFrom(params: URLSearchParams): number | null {
  const raw = params.get('id');
  if (raw === null) return null;

  const id = Number(raw);
  return Number.isInteger(id) && id > 0 ? id : null;
}

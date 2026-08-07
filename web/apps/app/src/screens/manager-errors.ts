import { ApiError } from '../api/index.js';
import type { ConflictDetails } from '../api/types.js';

export type ManagerSubject = 'account' | 'category' | 'rule';

/**
 * The `details` of a 409, or null for anything else.
 *
 * Narrow on purpose: a 409 whose details are missing or are not an object is
 * treated as an unstructured conflict, which falls through to the server's own
 * sentence rather than to a message invented from nothing.
 */
export function conflictDetailsOf(error: unknown): ConflictDetails | null {
  if (!(error instanceof ApiError) || error.status !== 409) return null;
  const details = error.details;
  if (typeof details !== 'object' || details === null) return null;
  return details as ConflictDetails;
}

function plural(count: number, noun: string): string {
  return `${count} ${noun}${count === 1 ? '' : 's'}`;
}

const SUBJECT_NOUNS: Record<ManagerSubject, string> = {
  account: 'account',
  category: 'category',
  rule: 'rule',
};

const SUBJECT_ARTICLES: Record<ManagerSubject, string> = {
  account: 'An',
  category: 'A',
  rule: 'A',
};

/**
 * What to tell the user about a refused write.
 *
 * The reason codes are the contract — `docs/api.md` says as much: "a client can
 * explain the block in its own words instead of parsing ours". So a reason we
 * know is answered from this table, with the count formatted here, and the
 * server's English is never shown. That is also what makes these strings the
 * only thing a translation would have to touch.
 *
 * Two deliberate exceptions:
 *
 * - A **400** renders the server's message. `Invalid regex: unclosed group` and
 *   `Invalid match type: fuzzy. Must be one of: contains, starts_with, regex`
 *   name the offending value and the legal set; re-deriving them here would
 *   produce a worse string that drifts from the server's actual rules.
 * - An **unrecognized 409 reason** also falls back to the message. Inventing
 *   "something conflicted" for a code we have not seen would hide the only
 *   information we have.
 */
export function guardrailMessage(error: unknown, subject: ManagerSubject): string {
  const details = conflictDetailsOf(error);

  if (details) {
    const count = details.count ?? 0;

    switch (details.reason) {
      case 'has_transactions':
        return subject === 'account'
          ? `This account has ${plural(count, 'transaction')}. Nigel will not delete an account that still has activity.`
          : `${plural(count, 'transaction')} ${count === 1 ? 'uses' : 'use'} this category. Recategorize them first.`;
      case 'has_active_rules':
        return `${plural(count, 'active rule')} ${count === 1 ? 'assigns' : 'assign'} this category. Delete those rules first.`;
      case 'duplicate_name':
        return details.name
          ? `${SUBJECT_ARTICLES[subject]} ${SUBJECT_NOUNS[subject]} named “${details.name}” already exists.`
          : `That ${SUBJECT_NOUNS[subject]} name is already taken.`;
      case 'already_inactive':
        return 'This rule has already been deleted. The list has been refreshed.';
    }
  }

  if (error instanceof ApiError) return error.message;
  return `Could not save that ${SUBJECT_NOUNS[subject]}.`;
}

export interface GuardrailAction {
  label: string;
  /** Query for `#/rules`. */
  params: URLSearchParams;
}

/**
 * The one guardrail that can point somewhere useful.
 *
 * "3 active rules assign this category" is a dead end on its own — the rules
 * are somewhere in a list sorted by priority, and finding them by eye is the
 * user's problem. The link filters the rules screen down to exactly them.
 */
export function guardrailAction(
  error: unknown,
  categoryId: number,
): GuardrailAction | null {
  const details = conflictDetailsOf(error);
  if (details?.reason !== 'has_active_rules') return null;
  return {
    label: 'Show those rules',
    params: new URLSearchParams({ categoryId: String(categoryId) }),
  };
}

/** A stale list rather than a real block: refetching is the whole fix. */
export function isStaleListConflict(error: unknown): boolean {
  return conflictDetailsOf(error)?.reason === 'already_inactive';
}

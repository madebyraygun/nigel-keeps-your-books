/**
 * The account types the data layer accepts, in the order the TUI offers them.
 *
 * Mirrors `ACCOUNT_TYPES` in `cli/accounts.rs`, which is the only place the
 * list is validated. Kept here rather than derived from the API because no
 * endpoint publishes it — a select has to name its options.
 */
export const ACCOUNT_TYPES = [
  'checking',
  'credit_card',
  'line_of_credit',
  'payroll',
] as const;

export type AccountTypeValue = (typeof ACCOUNT_TYPES)[number];

const ACCOUNT_TYPE_LABELS: Record<string, string> = {
  checking: 'Checking',
  credit_card: 'Credit card',
  line_of_credit: 'Line of credit',
  payroll: 'Payroll',
};

/**
 * The human name for an account type.
 *
 * The TUI prints the raw slug because it is fighting for columns; the web is
 * not. A value from outside the vocabulary falls back to itself rather than to
 * a guess, so a database written by some other tool still reads honestly.
 */
export function accountTypeLabel(value: string): string {
  return ACCOUNT_TYPE_LABELS[value] ?? value;
}

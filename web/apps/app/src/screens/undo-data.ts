import { transactionCountLabel, type ImportHistoryRow } from '@nigel/ui';

import type { ImportListItem, UndoneImport } from '../api/types.js';

/**
 * The history rows, as the component wants them.
 *
 * Order is the server's — `list_imports` is `ORDER BY i.id DESC`, so newest
 * first without the screen deciding anything.
 */
export function toImportRows(items: ImportListItem[]): ImportHistoryRow[] {
  return items.map((item) => ({
    id: item.id,
    filename: item.filename,
    accountName: item.accountName,
    importDate: item.importDate,
    transactionCount: item.transactionCount,
  }));
}

/**
 * What the confirmation asks.
 *
 * Names the count and the file, because those are the two facts that decide
 * the answer — and the count is the one that makes an accidental Enter
 * expensive.
 */
export function undoConfirmMessage(item: ImportHistoryRow): string {
  return `Delete ${transactionCountLabel(item.transactionCount)} imported from “${item.filename}”?`;
}

/** `undo.rs`'s sentence, verbatim, so both surfaces report the same thing. */
export function undoneMessage(filename: string, undone: UndoneImport): string {
  return `Rolled back import of “${filename}” (${undone.deletedTransactions} transactions removed)`;
}

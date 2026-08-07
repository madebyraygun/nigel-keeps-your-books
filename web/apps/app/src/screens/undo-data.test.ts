import { describe, it, expect } from 'vitest';
import type { ImportHistoryRow } from '@nigel/ui';

import {
  toImportRows,
  undoConfirmMessage,
  undoneMessage,
} from './undo-data.js';
import type { ImportListItem } from '../api/types.js';

const ITEMS: ImportListItem[] = [
  {
    id: 12,
    filename: 'march-checking.csv',
    accountName: 'BofA Checking',
    importDate: '2025-04-02 09:14:11',
    transactionCount: 42,
  },
  {
    id: 9,
    filename: 'january-checking.csv',
    accountName: 'BofA Checking',
    importDate: '2025-02-01 08:02:55',
    transactionCount: 0,
  },
];

describe('toImportRows', () => {
  it('keeps the server’s order rather than sorting again', () => {
    // list_imports is ORDER BY i.id DESC — newest first is already decided.
    expect(toImportRows(ITEMS).map((row) => row.id)).toEqual([12, 9]);
  });

  it('carries every field the row displays', () => {
    expect(toImportRows(ITEMS)[0]).toEqual({
      id: 12,
      filename: 'march-checking.csv',
      accountName: 'BofA Checking',
      importDate: '2025-04-02 09:14:11',
      transactionCount: 42,
    });
  });
});

describe('the two import-row shapes', () => {
  it('are assignable both ways, so they cannot drift apart', () => {
    // `@nigel/ui` depends on lit alone and may not import api types, so the
    // five fields are declared twice. Mutual assignability makes a field added
    // to one and not the other a compile error rather than a blank column.
    const apiToUi: ImportHistoryRow = ITEMS[0];
    const uiToApi: ImportListItem = apiToUi;

    expect(Object.keys(uiToApi).sort()).toEqual([
      'accountName',
      'filename',
      'id',
      'importDate',
      'transactionCount',
    ]);
  });
});

describe('undoConfirmMessage', () => {
  it('names the count and the file, which are what decide the answer', () => {
    expect(undoConfirmMessage(toImportRows(ITEMS)[0])).toBe(
      'Delete 42 transactions imported from “march-checking.csv”?',
    );
  });

  it('pluralizes at one', () => {
    const [one] = toImportRows([{ ...ITEMS[0], transactionCount: 1 }]);
    expect(undoConfirmMessage(one)).toBe(
      'Delete 1 transaction imported from “march-checking.csv”?',
    );
  });

  it('still asks about an import whose rows are already gone', () => {
    expect(undoConfirmMessage(toImportRows(ITEMS)[1])).toContain('0 transactions');
  });
});

describe('undoneMessage', () => {
  it('reports what undo.rs reports', () => {
    expect(
      undoneMessage('march-checking.csv', { id: 12, deletedTransactions: 42 }),
    ).toBe('Rolled back import of “march-checking.csv” (42 transactions removed)');
  });
});

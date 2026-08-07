import { describe, it, expect, afterEach, vi } from 'vitest';
import './undo.js';
import type { NigelUndoScreen } from './undo.js';
import type { NcToastDetail, WcImportHistory } from '@nigel/ui';

import { ApiError } from '../api/index.js';
import { FakeApiClient, notFoundError } from '../__mocks__/fake-api-client.js';
import type { ImportListItem } from '../api/types.js';

const IMPORTS: ImportListItem[] = [
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
    transactionCount: 3,
  },
];

function client(imports: ImportListItem[] = IMPORTS): FakeApiClient {
  const fake = new FakeApiClient();
  fake.imports = imports.map((item) => ({ ...item }));
  return fake;
}

async function settle(el: NigelUndoScreen): Promise<void> {
  await el.updateComplete;
  await new Promise((resolve) => setTimeout(resolve, 0));
  await el.updateComplete;
}

async function mount(
  fake: FakeApiClient = client(),
): Promise<{ el: NigelUndoScreen; fake: FakeApiClient }> {
  const el = document.createElement('nigel-undo-screen');
  el.client = fake;
  document.body.appendChild(el);
  await settle(el);
  return { el, fake };
}

function history(el: NigelUndoScreen): WcImportHistory {
  const table = el.shadowRoot?.querySelector('wc-import-history');
  if (!table) throw new Error('no wc-import-history rendered');
  return table as WcImportHistory;
}

async function answerConfirm(answer: boolean): Promise<void> {
  const ui = await import('@nigel/ui');
  vi.spyOn(ui, 'confirmDialog').mockResolvedValue(answer);
}

/** Toasts reach the shell on window, so that is where a test listens. */
function toasts(): NcToastDetail[] {
  const seen: NcToastDetail[] = [];
  window.addEventListener('nc-toast', (event) => {
    seen.push((event as CustomEvent<NcToastDetail>).detail);
  });
  return seen;
}

async function undoFirstRow(el: NigelUndoScreen): Promise<void> {
  history(el).dispatchEvent(
    new CustomEvent('nc-import-undo', { detail: { id: 12 }, bubbles: true }),
  );
  await settle(el);
}

describe('nigel-undo-screen', () => {
  afterEach(() => {
    document.body.innerHTML = '';
    vi.restoreAllMocks();
  });

  it('lists every import in the order the server sends them', async () => {
    const { el, fake } = await mount();

    expect(fake.calls).toContain('getImports');
    expect(history(el).imports.map((row) => row.id)).toEqual([12, 9]);
    expect(history(el).imports[0].filename).toBe('march-checking.csv');
  });

  it('asks before deleting anything, and does nothing when the answer is no', async () => {
    await answerConfirm(false);
    const { el, fake } = await mount();

    await undoFirstRow(el);

    expect(fake.calls.filter((call) => call.startsWith('deleteImport'))).toEqual([]);
    expect(history(el).imports).toHaveLength(2);
  });

  it('undoes the chosen import, reports the count, and refetches', async () => {
    await answerConfirm(true);
    const seen = toasts();
    const { el, fake } = await mount();

    await undoFirstRow(el);

    expect(fake.calls).toContain('deleteImport:12');
    expect(seen.at(-1)?.message).toBe(
      'Rolled back import of “march-checking.csv” (42 transactions removed)',
    );
    expect(seen.at(-1)?.variant).toBe('success');

    // Refetched rather than spliced: the other rows' counts are the server's.
    expect(fake.calls.filter((call) => call === 'getImports')).toHaveLength(2);
    expect(history(el).imports.map((row) => row.id)).toEqual([9]);
  });

  it('undoes the row that was asked for, not the newest one', async () => {
    await answerConfirm(true);
    const { el, fake } = await mount();

    history(el).dispatchEvent(
      new CustomEvent('nc-import-undo', { detail: { id: 9 }, bubbles: true }),
    );
    await settle(el);

    expect(fake.calls).toContain('deleteImport:9');
    expect(history(el).imports.map((row) => row.id)).toEqual([12]);
  });

  it('reports an import that went away in another tab, and refreshes', async () => {
    await answerConfirm(true);
    const seen = toasts();
    const fake = client();
    fake.deleteImportError = notFoundError('No import with ID 12');
    const { el } = await mount(fake);

    await undoFirstRow(el);

    expect(seen.at(-1)?.message).toBe('No import with ID 12');
    expect(seen.at(-1)?.variant).toBe('danger');
    // A stale list is exactly what a 404 means here, so it is reloaded.
    expect(fake.calls.filter((call) => call === 'getImports')).toHaveLength(2);
    expect(history(el).imports).toHaveLength(2);
  });

  it('says there is nothing to undo when no import has ever run', async () => {
    const { el } = await mount(client([]));
    expect(history(el).imports).toEqual([]);
  });

  it('offers a retry when the history will not load', async () => {
    const fake = client();
    fake.importsError = new ApiError({
      code: 'internal',
      rawCode: 'internal',
      message: 'Database is busy.',
      status: 500,
    });
    const { el } = await mount(fake);

    expect(history(el).error).toBe('Database is busy.');

    fake.importsError = null;
    history(el).dispatchEvent(new CustomEvent('nc-retry', { bubbles: true }));
    await settle(el);

    expect(history(el).error).toBeNull();
    expect(history(el).imports).toHaveLength(2);
  });
});

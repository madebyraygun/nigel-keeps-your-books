import { describe, it, expect, afterEach } from 'vitest';
import './wc-import-history.js';
import {
  transactionCountLabel,
  type ImportHistoryRow,
  type NcImportUndoDetail,
  type WcImportHistory,
} from './wc-import-history.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-import-history.preview.js';

const IMPORTS: ImportHistoryRow[] = [
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

async function mount(props: Partial<WcImportHistory> = {}): Promise<WcImportHistory> {
  const el = document.createElement('wc-import-history');
  Object.assign(el, { imports: IMPORTS }, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

afterEach(() => {
  document.body.innerHTML = '';
});

describe('transactionCountLabel', () => {
  it('pluralizes at one', () => {
    expect(transactionCountLabel(0)).toBe('0 transactions');
    expect(transactionCountLabel(1)).toBe('1 transaction');
    expect(transactionCountLabel(42)).toBe('42 transactions');
  });
});

describe('wc-import-history', () => {
  it('lists every import in the order it was given', async () => {
    const el = await mount();
    const rows = [...(el.shadowRoot?.querySelectorAll('tbody tr') ?? [])];

    expect(rows).toHaveLength(2);
    expect(rows[0].getAttribute('data-row')).toBe('12');
    expect(rows[0].textContent).toContain('march-checking.csv');
    expect(rows[0].textContent).toContain('BofA Checking');
    expect(rows[0].textContent).toContain('2025-04-02 09:14:11');
  });

  it('keeps an import whose transactions are already gone', async () => {
    // list_imports is a LEFT JOIN: a zero count is a real row, not an absence.
    const el = await mount();
    const rows = [...(el.shadowRoot?.querySelectorAll('tbody tr') ?? [])];
    expect(rows[1].querySelector('.count')?.textContent?.trim()).toBe('0');
  });

  it('names the file in each button, so a screen reader gets more than "Undo"', async () => {
    const el = await mount();
    const button = el.shadowRoot?.querySelector('tbody tr [data-undo]');
    expect(button?.getAttribute('aria-label')).toBe('Undo import of march-checking.csv');
  });

  it('reports which import to undo', async () => {
    const el = await mount();
    let detail: NcImportUndoDetail | null = null;
    el.addEventListener('nc-import-undo', (event) => {
      detail = (event as CustomEvent<NcImportUndoDetail>).detail;
    });

    const button = el.shadowRoot?.querySelector('tbody tr [data-undo]') as HTMLElement;
    button.click();

    expect(detail).toEqual({ id: 12 });
  });

  it('makes the busy row inert without disabling the rest', async () => {
    const el = await mount({ busyId: 12 });
    const rows = [...(el.shadowRoot?.querySelectorAll('tbody tr') ?? [])];

    expect(rows[0].getAttribute('aria-busy')).toBe('true');
    expect(rows[0].querySelector('[data-undo]')?.hasAttribute('disabled')).toBe(true);
    expect(rows[1].querySelector('[data-undo]')?.hasAttribute('disabled')).toBe(false);
  });

  it('says there is nothing to undo, in the TUI’s words', async () => {
    const el = await mount({ imports: [] });
    const empty = el.shadowRoot?.querySelector('wc-empty-state');
    expect(empty?.getAttribute('message')).toBe('No imports to undo.');
    expect(el.shadowRoot?.querySelector('table')).toBeNull();
  });

  it('offers a retry instead of the table when the load failed', async () => {
    const el = await mount({ error: 'Could not load the import history.' });
    let retried = false;
    el.addEventListener('nc-retry', () => {
      retried = true;
    });

    const notice = el.shadowRoot?.querySelector('wc-notice-bar');
    expect(notice?.getAttribute('message')).toBe('Could not load the import history.');
    notice?.dispatchEvent(new CustomEvent('nc-notice-action'));
    expect(retried).toBe(true);
  });

  it('shows a spinner while loading', async () => {
    const el = await mount({ loading: true });
    expect(el.shadowRoot?.querySelector('wc-spinner')).not.toBeNull();
  });
});

describePreviewA11y(preview);

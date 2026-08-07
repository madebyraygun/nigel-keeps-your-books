import { describe, it, expect, afterEach } from 'vitest';
import './wc-reconciliation-history.js';
import type {
  ReconciliationHistoryRow,
  WcReconciliationHistory,
} from './wc-reconciliation-history.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-reconciliation-history.preview.js';

const ROWS: ReconciliationHistoryRow[] = [
  {
    id: 4,
    month: '2025-03',
    statementBalance: 5000,
    calculatedBalance: 4871.44,
    isReconciled: false,
    reconciledAt: null,
  },
  {
    id: 3,
    month: '2025-02',
    statementBalance: 4928.01,
    calculatedBalance: 4928.01,
    isReconciled: true,
    reconciledAt: '2025-03-01 12:04:18',
  },
  {
    id: 1,
    month: '2024-12',
    statementBalance: null,
    calculatedBalance: null,
    isReconciled: true,
    reconciledAt: '2025-01-04 10:00:00',
  },
];

async function mount(
  props: Partial<WcReconciliationHistory> = {},
): Promise<WcReconciliationHistory> {
  const el = document.createElement('wc-reconciliation-history');
  Object.assign(el, { rows: ROWS }, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

afterEach(() => {
  document.body.innerHTML = '';
});

describe('wc-reconciliation-history', () => {
  it('keeps the order it was given, which is the server’s newest-first', async () => {
    const el = await mount();
    const months = [...(el.shadowRoot?.querySelectorAll('tbody .month') ?? [])].map(
      (node) => node.textContent?.trim(),
    );
    expect(months).toEqual(['2025-03', '2025-02', '2024-12']);
  });

  it('shows a mismatch as a recorded result rather than hiding it', async () => {
    const el = await mount();
    const first = el.shadowRoot?.querySelector('tbody tr');
    expect(first?.querySelector('.status')?.textContent?.trim()).toBe('✗ Discrepancy');
    expect(first?.querySelector('.status')?.classList.contains('off')).toBe(true);
  });

  it('marks a reconciled month and shows when it was checked', async () => {
    const el = await mount();
    const rows = [...(el.shadowRoot?.querySelectorAll('tbody tr') ?? [])];
    expect(rows[1].querySelector('.status')?.textContent?.trim()).toBe('✓ Reconciled');
    expect(rows[1].textContent).toContain('2025-03-01 12:04:18');
  });

  it('renders a missing balance as an em dash, never as zero', async () => {
    // Both balance columns are nullable: a record can predate either figure,
    // and "$0.00" would be a number the database never held.
    const el = await mount();
    const rows = [...(el.shadowRoot?.querySelectorAll('tbody tr') ?? [])];
    const amounts = [...rows[2].querySelectorAll('.amount')];

    expect(amounts.map((cell) => cell.textContent?.trim())).toEqual(['—', '—']);
    expect(rows[2].querySelectorAll('wc-money')).toHaveLength(0);
  });

  it('renders a stored balance through wc-money', async () => {
    const el = await mount();
    const first = el.shadowRoot?.querySelector('tbody tr');
    expect(first?.querySelectorAll('wc-money')).toHaveLength(2);
  });

  it('shows an unstamped record’s checked column as an em dash', async () => {
    const el = await mount();
    const first = el.shadowRoot?.querySelector('tbody tr');
    const cells = [...(first?.querySelectorAll('td') ?? [])];
    expect(cells[cells.length - 1].textContent?.trim()).toBe('—');
  });

  it('says nothing has been checked yet when the list is empty', async () => {
    const el = await mount({ rows: [] });
    expect(el.shadowRoot?.querySelector('wc-empty-state')).not.toBeNull();
    expect(el.shadowRoot?.querySelector('table')).toBeNull();
  });

  it('offers a retry when the load failed', async () => {
    const el = await mount({ error: 'Could not load past reconciliations.' });
    let retried = false;
    el.addEventListener('nc-retry', () => {
      retried = true;
    });

    el.shadowRoot
      ?.querySelector('wc-notice-bar')
      ?.dispatchEvent(new CustomEvent('nc-notice-action'));
    expect(retried).toBe(true);
  });

  it('shows a spinner while loading', async () => {
    const el = await mount({ loading: true });
    expect(el.shadowRoot?.querySelector('wc-spinner')).not.toBeNull();
  });
});

describePreviewA11y(preview);

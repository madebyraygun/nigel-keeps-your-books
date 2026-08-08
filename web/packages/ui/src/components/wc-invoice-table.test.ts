import { describe, it, expect, afterEach } from 'vitest';
import './wc-invoice-table.js';
import type { InvoiceTableRow, WcInvoiceTable } from './wc-invoice-table.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-invoice-table.preview.js';

const ROWS: InvoiceTableRow[] = [
  {
    number: 1250,
    status: 'partial',
    clientName: 'Acme Co',
    total: 3200,
    balance: 1200,
    dueDate: '2026-08-20',
    href: '#/invoices?number=1250',
  },
  {
    number: 1247,
    status: 'void',
    clientName: null,
    total: 500,
    balance: null,
    dueDate: null,
  },
];

async function mount(props: Partial<WcInvoiceTable> = {}): Promise<WcInvoiceTable> {
  const el = document.createElement('wc-invoice-table');
  Object.assign(el, { rows: ROWS }, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('wc-invoice-table', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders one row per invoice, keyed by number', async () => {
    const el = await mount();
    const numbers = [...(el.shadowRoot?.querySelectorAll('tr[data-row]') ?? [])].map(
      (tr) => tr.getAttribute('data-row'),
    );
    expect(numbers).toEqual(['1250', '1247']);
  });

  it('shows an em dash for a void balance rather than $0.00', async () => {
    // A void invoice owes nothing and never will; `$0.00` reads as settled,
    // which is a different thing. The `wc-import-history` null precedent.
    const el = await mount();
    const cells = [...(el.shadowRoot?.querySelectorAll('[data-balance]') ?? [])];
    expect(cells[0].querySelector('wc-money')).toBeTruthy();
    expect(cells[1].querySelector('wc-money')).toBeNull();
    expect(cells[1].textContent?.trim()).toBe('—');
  });

  it('shows an em dash for a missing client and a missing due date', async () => {
    const el = await mount();
    const row = el.shadowRoot?.querySelector('tr[data-row="1247"]');
    const cells = [...(row?.querySelectorAll('td') ?? [])].map((td) =>
      td.textContent?.trim(),
    );
    expect(cells[2]).toBe('—');
    expect(cells[5]).toBe('—');
  });

  it('links a row only when given an address', async () => {
    const el = await mount();
    const links = [...(el.shadowRoot?.querySelectorAll('a') ?? [])].map((a) =>
      a.getAttribute('href'),
    );
    expect(links).toEqual(['#/invoices?number=1250']);
  });

  it('carries the status word into a status chip', async () => {
    const el = await mount();
    const chips = [...(el.shadowRoot?.querySelectorAll('wc-invoice-status') ?? [])].map(
      (chip) => chip.getAttribute('status'),
    );
    expect(chips).toEqual(['partial', 'void']);
  });

  it('says it is loading before it says it is empty', async () => {
    const el = await mount({ rows: [], loading: true });
    expect(el.shadowRoot?.querySelector('wc-spinner')).toBeTruthy();
    expect(el.shadowRoot?.querySelector('[data-empty]')).toBeNull();
  });

  it('renders the empty message it was given', async () => {
    const el = await mount({ rows: [], emptyMessage: 'No invoices yet.' });
    expect(el.shadowRoot?.querySelector('[data-empty]')?.textContent).toContain(
      'No invoices yet.',
    );
  });
});

describePreviewA11y(preview);

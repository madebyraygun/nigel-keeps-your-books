import { describe, it, expect, afterEach } from 'vitest';
import './wc-invoice-summary.js';
import type { WcInvoiceSummary } from './wc-invoice-summary.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-invoice-summary.preview.js';

async function mount(props: Partial<WcInvoiceSummary> = {}): Promise<WcInvoiceSummary> {
  const el = document.createElement('wc-invoice-summary');
  Object.assign(
    el,
    {
      number: 1250,
      status: 'partial',
      clientName: 'Acme Co',
      total: 3200,
      balance: 1200,
      issueDate: '2026-02-20',
      dueDate: '2026-03-20',
    },
    props,
  );
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function amountOf(el: WcInvoiceSummary, hook: string): number | undefined {
  const money = el.shadowRoot?.querySelector(hook) as
    | (HTMLElement & { amount: number })
    | null;
  return money?.amount;
}

describe('wc-invoice-summary', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('leads with the number, the status and the client', async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelector('h2')?.textContent).toContain('#1250');
    expect(el.shadowRoot?.querySelector('wc-invoice-status')?.getAttribute('status')).toBe(
      'partial',
    );
    expect(el.shadowRoot?.querySelector('.client')?.textContent?.trim()).toBe('Acme Co');
  });

  it('shows the total and what is still outstanding, not one of them', async () => {
    const el = await mount();
    expect(amountOf(el, '[data-total]')).toBe(3200);
    expect(amountOf(el, '[data-balance]')).toBe(1200);
  });

  it('renders an em dash for an invoice with no due date', async () => {
    const el = await mount({ dueDate: null });
    expect(el.shadowRoot?.querySelector('[data-due]')?.textContent?.trim()).toBe('—');
  });

  it('renders an em dash for an invoice whose client row is gone', async () => {
    const el = await mount({ clientName: null });
    expect(el.shadowRoot?.querySelector('.client')?.textContent?.trim()).toBe('—');
  });

  it('passes the currency down to the figures', async () => {
    const el = await mount({ currency: 'EUR' });
    const money = el.shadowRoot?.querySelector('[data-total]') as HTMLElement & {
      currency: string;
    };
    expect(money.currency).toBe('EUR');
  });
});

describePreviewA11y(preview);

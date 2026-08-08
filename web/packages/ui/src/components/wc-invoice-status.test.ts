import { describe, it, expect, afterEach } from 'vitest';
import './wc-invoice-status.js';
import {
  INVOICE_STATUS_WORDS,
  type WcInvoiceStatus,
} from './wc-invoice-status.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-invoice-status.preview.js';

async function mount(status: string): Promise<WcInvoiceStatus> {
  const el = document.createElement('wc-invoice-status');
  el.status = status;
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('wc-invoice-status', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('knows the six statuses the data layer derives', () => {
    expect(INVOICE_STATUS_WORDS).toEqual([
      'draft',
      'sent',
      'partial',
      'paid',
      'overdue',
      'void',
    ]);
  });

  it.each(INVOICE_STATUS_WORDS)('renders %s as a glyph and the word', async (status) => {
    const el = await mount(status);
    const chip = el.shadowRoot?.querySelector('.chip');
    expect(chip?.getAttribute('data-status')).toBe(status);
    expect(chip?.querySelector('.word')?.textContent).toBe(status);
    // The glyph is decoration: the word is what a screen reader announces.
    expect(chip?.querySelector('.glyph')?.getAttribute('aria-hidden')).toBe('true');
    expect(chip?.querySelector('.glyph')?.textContent?.trim()).not.toBe('');
  });

  it('renders a status it has never seen rather than blanking it', async () => {
    // `invoices.status` has no CHECK constraint, so a row written by the
    // InvoiceShelf importer or by hand cannot be assumed to be one of the six.
    const el = await mount('imported');
    expect(el.shadowRoot?.querySelector('.word')?.textContent).toBe('imported');
    expect(el.shadowRoot?.querySelector('.glyph')?.textContent?.trim()).toBe('•');
  });
});

describePreviewA11y(preview);

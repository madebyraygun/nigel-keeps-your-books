import { describe, it, expect, afterEach } from 'vitest';
import './wc-invoice-form.js';
import {
  EMPTY_INVOICE_FORM,
  invoiceFormItems,
  validateInvoiceForm,
  type InvoiceClientOption,
  type InvoiceFormValue,
  type NcInvoiceFormChangeDetail,
  type WcInvoiceForm,
} from './wc-invoice-form.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-invoice-form.preview.js';

const CLIENTS: InvoiceClientOption[] = [
  { id: 1, name: 'Acme Co', email: 'ap@acme.test' },
  { id: 2, name: 'Globex', email: null },
];

const valid: InvoiceFormValue = {
  clientId: '1',
  issueDate: '2026-08-07',
  dueDate: '2026-09-06',
  currency: 'USD',
  notes: '',
  terms: '',
  items: [{ description: 'Consulting', quantity: '10', unitAmount: '150' }],
};

async function mount(props: Partial<WcInvoiceForm> = {}): Promise<WcInvoiceForm> {
  const el = document.createElement('wc-invoice-form');
  Object.assign(el, { value: valid, clients: CLIENTS }, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('invoiceFormItems', () => {
  it('drops the rows nobody typed into', () => {
    const items = invoiceFormItems({
      ...valid,
      items: [
        valid.items[0],
        { description: '', quantity: '1', unitAmount: '' },
        { description: 'Hosting', quantity: '1', unitAmount: '350' },
      ],
    });
    expect(items.map((item) => item.description)).toEqual(['Consulting', 'Hosting']);
  });
});

describe('validateInvoiceForm', () => {
  it('accepts a well-formed invoice', () => {
    expect(validateInvoiceForm(valid)).toEqual({});
  });

  it('requires a client', () => {
    expect(validateInvoiceForm({ ...valid, clientId: '' }).clientId).toBe(
      'Choose a client',
    );
  });

  it('holds dates to the API shape, not the CLI one', () => {
    // `validate_date` accepts `2026-4-1`; the route does not, and a form that
    // accepted it would send a request it knows will be a 400.
    expect(validateInvoiceForm({ ...valid, issueDate: '2026-8-7' }).issueDate).toBe(
      'Issue date must be YYYY-MM-DD',
    );
    expect(validateInvoiceForm({ ...valid, issueDate: '' }).issueDate).toContain(
      'required',
    );
  });

  it('treats an empty due date as no due date rather than an error', () => {
    expect(validateInvoiceForm({ ...valid, dueDate: '' }).dueDate).toBeUndefined();
    expect(validateInvoiceForm({ ...valid, dueDate: '2026-9-6' }).dueDate).toBe(
      'Due date must be YYYY-MM-DD',
    );
  });

  it('requires a three-letter currency code', () => {
    expect(validateInvoiceForm({ ...valid, currency: 'DOLLARS' }).currency).toContain(
      'three-letter',
    );
  });

  it('refuses an invoice with no lines at all', () => {
    expect(validateInvoiceForm({ ...valid, items: [] }).items).toContain(
      'at least one line item',
    );
    expect(
      validateInvoiceForm({
        ...valid,
        items: [{ description: '', quantity: '1', unitAmount: '' }],
      }).items,
    ).toContain('at least one line item');
  });

  it('reports per-row problems in row order', () => {
    const errors = validateInvoiceForm({
      ...valid,
      items: [
        { description: '', quantity: 'lots', unitAmount: '150' },
        valid.items[0],
      ],
    });
    expect(errors.itemErrors?.[0]).toEqual({
      description: 'Description is required',
      quantity: 'Quantity must be a number',
    });
    expect(errors.itemErrors?.[1]).toEqual({});
  });

  it('refuses a line that overflows and a total of zero', () => {
    // `validate_items` checks the arithmetic's result, not its inputs: two
    // huge finite figures multiply to infinity, which serde renders as null.
    expect(
      validateInvoiceForm({
        ...valid,
        items: [{ description: 'Big', quantity: '1e308', unitAmount: '1e308' }],
      }).itemErrors?.[0].unitAmount,
    ).toContain('too large');

    expect(
      validateInvoiceForm({
        ...valid,
        items: [{ description: 'Free', quantity: '0', unitAmount: '150' }],
      }).items,
    ).toContain('more than zero');
  });
});

describe('wc-invoice-form', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('collects every field the create route takes', async () => {
    const el = await mount({ value: EMPTY_INVOICE_FORM });
    for (const hook of [
      '[data-client]',
      '[data-issue]',
      '[data-due]',
      '[data-currency]',
      '[data-items]',
      '[data-notes]',
      '[data-terms]',
    ]) {
      expect(el.shadowRoot?.querySelector(hook), hook).toBeTruthy();
    }
  });

  it('emits the whole value on every edit', async () => {
    const el = await mount();
    const seen: InvoiceFormValue[] = [];
    el.addEventListener('nc-invoice-form-change', (event) =>
      seen.push((event as CustomEvent<NcInvoiceFormChangeDetail>).detail.value),
    );

    const input = el.shadowRoot?.querySelector<HTMLInputElement>('[data-due]');
    input!.value = '2026-10-01';
    input!.dispatchEvent(new Event('input'));

    expect(seen).toEqual([{ ...valid, dueDate: '2026-10-01' }]);
  });

  it('carries a line-item edit up as the whole value', async () => {
    const el = await mount();
    const seen: InvoiceFormValue[] = [];
    el.addEventListener('nc-invoice-form-change', (event) =>
      seen.push((event as CustomEvent<NcInvoiceFormChangeDetail>).detail.value),
    );

    el.shadowRoot
      ?.querySelector('[data-items]')
      ?.shadowRoot?.querySelector<HTMLElement>('[data-add-row]')
      ?.click();

    expect(seen[0].items).toHaveLength(2);
  });

  it('warns when the chosen client has no email', async () => {
    const el = await mount({ value: { ...valid, clientId: '2' } });
    expect(el.shadowRoot?.querySelector('[data-no-email]')?.textContent).toContain(
      'cannot be sent',
    );
  });

  it('locks the client on an edit — an invoice stays with the client it was raised for', async () => {
    const el = await mount({ mode: 'edit' });
    expect(el.shadowRoot?.querySelector('[data-client]')?.hasAttribute('disabled')).toBe(
      true,
    );
  });

  it('renders the list-level refusal on the line items rather than beside a field', async () => {
    const el = await mount({
      value: { ...valid, items: [] },
      errors: { items: 'An invoice needs at least one line item.' },
    });
    expect(el.shadowRoot?.querySelector('[data-items]')?.getAttribute('list-error')).toBe(
      'An invoice needs at least one line item.',
    );
  });

  it('disables every control while a save is in flight', async () => {
    const el = await mount({ disabled: true });
    const controls = [
      ...(el.shadowRoot?.querySelectorAll(
        '[data-client],[data-issue],[data-due],[data-currency],[data-notes],[data-terms]',
      ) ?? []),
    ];
    expect(controls).toHaveLength(6);
    expect(controls.every((control) => control.hasAttribute('disabled'))).toBe(true);
  });
});

describePreviewA11y(preview);

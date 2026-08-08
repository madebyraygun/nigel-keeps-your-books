import { describe, it, expect } from 'vitest';
import { EMPTY_INVOICE_FORM, type InvoiceFormValue } from '@nigel/ui';

import {
  activeStatusFilter,
  clientFormFrom,
  clientPatch,
  detailLineItems,
  invoiceFormFrom,
  invoiceListParams,
  invoicePatch,
  invoiceTableRows,
  isEmptyPatch,
  newClientRequest,
  newInvoiceRequest,
  payRequest,
  sendStepViews,
  today,
} from './invoice-data.js';
import type { Client, InvoiceDetail, InvoiceListRow } from '../api/types.js';

const CLIENT: Client = {
  id: 1,
  name: 'Acme Co',
  email: 'ap@acme.test',
  billingAddress: '1 Main St',
  notes: null,
};

const DETAIL: InvoiceDetail = {
  id: 3,
  number: 1250,
  clientId: 1,
  status: 'draft',
  currency: 'USD',
  issueDate: '2026-02-20',
  dueDate: '2026-03-20',
  subtotal: 3200,
  tax: 0,
  total: 3200,
  notes: null,
  terms: 'Net 30',
  stripePaymentLinkId: null,
  stripePaymentLinkUrl: null,
  publishedAt: null,
  voidedAt: null,
  client: CLIENT,
  items: [
    {
      id: 1,
      invoiceId: 3,
      description: 'Consulting - February',
      quantity: 16,
      unitAmount: 175,
      lineTotal: 2800,
      position: 0,
    },
    {
      id: 2,
      invoiceId: 3,
      description: 'Hosting',
      quantity: 1,
      unitAmount: 400,
      lineTotal: 400,
      position: 1,
    },
  ],
  payments: [],
  paid: 0,
  balance: 3200,
  publicUrl: null,
  canEdit: true,
  canSend: true,
  canVoid: true,
  canPay: true,
};

const FORM: InvoiceFormValue = invoiceFormFrom(DETAIL);

describe('invoiceListParams', () => {
  it('omits status when the filter is "all"', () => {
    // `all` is not one of the server's words: a status it does not know is a
    // 400 naming the legal set, not an ignored parameter.
    expect(invoiceListParams(new URLSearchParams('status=all'))).toEqual({});
    expect(invoiceListParams(new URLSearchParams())).toEqual({});
  });

  it('passes a real status through', () => {
    expect(invoiceListParams(new URLSearchParams('status=open'))).toEqual({
      status: 'open',
    });
  });

  it('carries clientId only when one is chosen', () => {
    expect(invoiceListParams(new URLSearchParams('clientId=3'))).toEqual({ clientId: 3 });
    expect(invoiceListParams(new URLSearchParams('clientId=nope'))).toEqual({});
    expect(invoiceListParams(new URLSearchParams('clientId=0'))).toEqual({});
  });

  it('reports which chip is on for a route that names none', () => {
    expect(activeStatusFilter(new URLSearchParams())).toBe('all');
    expect(activeStatusFilter(new URLSearchParams('status=paid'))).toBe('paid');
  });
});

describe('today', () => {
  it('reads the local day, not the UTC one', () => {
    // A user in UTC-5 filing a payment at 20:00 means their date, not
    // tomorrow's — the same reasoning `indexOfToday` keeps in the register.
    expect(today(new Date(2026, 2, 5, 20, 30))).toBe('2026-03-05');
  });
});

describe('newInvoiceRequest', () => {
  it('builds the request the create route takes', () => {
    expect(newInvoiceRequest(FORM)).toEqual({
      clientId: 1,
      issueDate: '2026-02-20',
      dueDate: '2026-03-20',
      currency: 'USD',
      items: [
        { description: 'Consulting - February', quantity: 16, unitAmount: 175 },
        { description: 'Hosting', quantity: 1, unitAmount: 400 },
      ],
      terms: 'Net 30',
    });
  });

  it('drops empty trailing rows', () => {
    const request = newInvoiceRequest({
      ...FORM,
      items: [...FORM.items, { description: '', quantity: '1', unitAmount: '' }],
    });
    expect(request?.items).toHaveLength(2);
  });

  it('omits an empty due date rather than sending null on a create', () => {
    expect(newInvoiceRequest({ ...FORM, dueDate: '' })).not.toHaveProperty('dueDate');
  });

  it('refuses a total of zero before the request is made', () => {
    expect(
      newInvoiceRequest({
        ...FORM,
        items: [{ description: 'Free', quantity: '0', unitAmount: '150' }],
      }),
    ).toBeNull();
  });

  it('refuses a form with no client or a malformed date', () => {
    expect(newInvoiceRequest({ ...FORM, clientId: '' })).toBeNull();
    expect(newInvoiceRequest({ ...FORM, issueDate: '2026-2-20' })).toBeNull();
  });

  it('upper-cases the currency the way validate_currency does', () => {
    expect(newInvoiceRequest({ ...FORM, currency: 'eur' })?.currency).toBe('EUR');
  });
});

describe('invoiceFormFrom', () => {
  it('round-trips an invoice into the editor and back with no patch', () => {
    expect(invoicePatch(DETAIL, invoiceFormFrom(DETAIL))).toEqual({});
    expect(isEmptyPatch(invoicePatch(DETAIL, invoiceFormFrom(DETAIL)))).toBe(true);
  });

  it('renders an absent due date, note and term as empty fields', () => {
    const form = invoiceFormFrom({ ...DETAIL, dueDate: null, terms: null });
    expect(form.dueDate).toBe('');
    expect(form.terms).toBe('');
  });

  it('starts from the empty form, so a new field cannot arrive undefined', () => {
    expect(Object.keys(invoiceFormFrom(DETAIL)).sort()).toEqual(
      Object.keys(EMPTY_INVOICE_FORM).sort(),
    );
  });
});

describe('invoicePatch', () => {
  it('sends only changed fields — an all-absent PATCH is a 400', () => {
    expect(invoicePatch(DETAIL, { ...FORM, issueDate: '2026-02-21' })).toEqual({
      issueDate: '2026-02-21',
    });
  });

  it('sends dueDate: null to clear it, and omits it when unchanged', () => {
    expect(invoicePatch(DETAIL, { ...FORM, dueDate: '' })).toEqual({ dueDate: null });
    expect(invoicePatch(DETAIL, FORM)).not.toHaveProperty('dueDate');
  });

  it('clears notes and terms with null too', () => {
    expect(invoicePatch(DETAIL, { ...FORM, terms: '' })).toEqual({ terms: null });
    expect(invoicePatch(DETAIL, { ...FORM, notes: 'Thanks' })).toEqual({
      notes: 'Thanks',
    });
  });

  it('sends the whole items array when any row changed', () => {
    const patch = invoicePatch(DETAIL, {
      ...FORM,
      items: [FORM.items[0], { ...FORM.items[1], unitAmount: '450' }],
    });
    expect(patch.items).toEqual([
      { description: 'Consulting - February', quantity: 16, unitAmount: 175 },
      { description: 'Hosting', quantity: 1, unitAmount: 450 },
    ]);
  });

  it('sends the whole array when a row is removed or reordered', () => {
    expect(invoicePatch(DETAIL, { ...FORM, items: [FORM.items[0]] }).items).toHaveLength(1);
    expect(
      invoicePatch(DETAIL, { ...FORM, items: [FORM.items[1], FORM.items[0]] }).items?.[0]
        .description,
    ).toBe('Hosting');
  });

  it('does not send items when only their blank neighbours changed', () => {
    const patch = invoicePatch(DETAIL, {
      ...FORM,
      items: [...FORM.items, { description: '', quantity: '1', unitAmount: '' }],
    });
    expect(patch).not.toHaveProperty('items');
  });
});

describe('payRequest', () => {
  it('omits the amount when the field is empty, which means the whole balance', () => {
    expect(payRequest({ amount: '', date: '2026-03-15', method: 'ach' })).toEqual({
      date: '2026-03-15',
      method: 'ach',
    });
  });

  it('sends a typed amount with its commas stripped', () => {
    expect(
      payRequest({ amount: '1,200.50', date: '2026-03-15', method: 'direct_deposit' }),
    ).toEqual({ amount: 1200.5, date: '2026-03-15', method: 'direct_deposit' });
  });

  it('omits a method the CHECK constraint would refuse rather than sending it', () => {
    // The route would answer 400 naming the legal set; letting the server's
    // default stand is the honest thing for a value no control can produce.
    expect(payRequest({ amount: '', date: '2026-03-15', method: 'bitcoin' })).toEqual({
      date: '2026-03-15',
    });
  });
});

describe('client requests', () => {
  it('sends an empty optional field as null, not as an empty string', () => {
    expect(
      newClientRequest({ name: ' Acme Co ', email: '', billingAddress: '', notes: '' }),
    ).toEqual({ name: 'Acme Co', email: null, billingAddress: null, notes: null });
  });

  it('patches only what moved, and clears with null', () => {
    const form = clientFormFrom(CLIENT);
    expect(clientPatch(CLIENT, form)).toEqual({});
    expect(clientPatch(CLIENT, { ...form, email: '' })).toEqual({ email: null });
    expect(clientPatch(CLIENT, { ...form, name: 'Acme Corp' })).toEqual({
      name: 'Acme Corp',
    });
  });
});

describe('invoiceTableRows', () => {
  const rows: InvoiceListRow[] = [
    {
      id: 1,
      number: 1250,
      status: 'partial',
      clientId: 1,
      clientName: 'Acme Co',
      issueDate: '2026-02-20',
      dueDate: '2026-03-20',
      currency: 'USD',
      total: 3200,
      paid: 2000,
      balance: 1200,
    },
    {
      id: 2,
      number: 1247,
      status: 'void',
      clientId: 2,
      clientName: null,
      issueDate: '2026-01-05',
      dueDate: null,
      currency: 'USD',
      total: 500,
      paid: 0,
      balance: 500,
    },
  ];

  it('drops a void invoice’s balance so the table shows an em dash', () => {
    const mapped = invoiceTableRows(rows);
    expect(mapped[0].balance).toBe(1200);
    expect(mapped[1].balance).toBeNull();
  });

  it('links each row at its own number', () => {
    expect(invoiceTableRows(rows)[0].href).toBe('#/invoices?number=1250');
  });

  it('keeps a null client name rather than inventing one', () => {
    expect(invoiceTableRows(rows)[1].clientName).toBeNull();
  });
});

describe('detailLineItems', () => {
  it('prints quantities to two decimals, as format_invoice_show does', () => {
    expect(detailLineItems(DETAIL).map((item) => item.quantity)).toEqual(['16.00', '1.00']);
  });
});

describe('sendStepViews', () => {
  it('shows every step, not only the ones that ran', () => {
    const views = sendStepViews({ completed: ['config', 'load'], running: 'precheck' });
    expect(views).toHaveLength(8);
    expect(views.map((view) => view.state)).toEqual([
      'ok',
      'ok',
      'running',
      'pending',
      'pending',
      'pending',
      'pending',
      'pending',
    ]);
  });

  it('marks a reused payment link as reused rather than as fresh work', () => {
    const views = sendStepViews({
      completed: [
        { step: 'config', outcome: 'ok' },
        { step: 'payment_link', outcome: 'reused' },
      ],
    });
    expect(views.find((view) => view.step === 'payment_link')?.state).toBe('reused');
  });

  it('marks the failed step and leaves everything after it pending', () => {
    const views = sendStepViews({
      completed: ['config', 'load', 'precheck', 'payment_link', 'render'],
      failed: 'publish',
    });
    expect(views.find((view) => view.step === 'publish')?.state).toBe('failed');
    expect(views.find((view) => view.step === 'email')?.state).toBe('pending');
  });

  it('gives every step a label in our own words', () => {
    expect(sendStepViews({}).every((view) => view.label.length > 0)).toBe(true);
  });
});

import { html } from 'lit';
import './wc-invoice-form.js';
import { EMPTY_INVOICE_FORM, type InvoiceClientOption } from './wc-invoice-form.js';
import type { Preview } from '../../preview/types.js';

const clients: InvoiceClientOption[] = [
  { id: 1, name: 'Acme Co', email: 'ap@acme.test' },
  { id: 2, name: 'Globex', email: null },
  { id: 3, name: 'Northwind Traders', email: 'billing@nw.test' },
];

const filled = {
  clientId: '1',
  issueDate: '2026-08-07',
  dueDate: '2026-09-06',
  currency: 'USD',
  notes: 'Thanks for your business.',
  terms: 'Net 30',
  items: [
    { description: 'Consulting — August', quantity: '10', unitAmount: '150' },
    { description: 'Hosting', quantity: '1', unitAmount: '350' },
  ],
};

const preview: Preview = {
  id: 'wc-invoice-form',
  title: 'Invoice form',
  group: 'Invoicing',
  description:
    'Client, dates, currency, notes, terms and the repeatable line items. A full view rather than a dialog — an invoice with eight lines inside a dialog is a scrolling box inside a scrolling page.',
  layout: 'stack',
  states: [
    {
      name: 'new',
      render: () => html`
        <wc-invoice-form
          .value=${{ ...EMPTY_INVOICE_FORM, issueDate: '2026-08-07' }}
          .clients=${clients}
        ></wc-invoice-form>
      `,
    },
    {
      name: 'editing',
      render: () => html`
        <wc-invoice-form mode="edit" .value=${filled} .clients=${clients}></wc-invoice-form>
      `,
    },
    {
      name: 'client-without-email',
      render: () => html`
        <wc-invoice-form
          .value=${{ ...filled, clientId: '2' }}
          .clients=${clients}
        ></wc-invoice-form>
      `,
    },
    {
      name: 'validation-errors',
      render: () => html`
        <wc-invoice-form
          .value=${{
            ...filled,
            clientId: '',
            issueDate: '2026-8-7',
            currency: 'DOLLARS',
            items: [{ description: '', quantity: 'lots', unitAmount: '' }],
          }}
          .clients=${clients}
          .errors=${{
            clientId: 'Choose a client',
            issueDate: 'Issue date must be YYYY-MM-DD',
            currency: 'Currency must be a three-letter code',
            itemErrors: [
              {
                description: 'Description is required',
                quantity: 'Quantity must be a number',
                unitAmount: 'Unit amount must be a number',
              },
            ],
          }}
        ></wc-invoice-form>
      `,
    },
    {
      name: 'no-line-items',
      render: () => html`
        <wc-invoice-form
          .value=${{ ...filled, items: [] }}
          .clients=${clients}
          .errors=${{ items: 'An invoice needs at least one line item.' }}
        ></wc-invoice-form>
      `,
    },
    {
      name: 'saving',
      render: () => html`
        <wc-invoice-form .value=${filled} .clients=${clients} disabled></wc-invoice-form>
      `,
    },
  ],
};

export default preview;

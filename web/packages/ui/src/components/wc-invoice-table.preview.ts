import { html } from 'lit';
import './wc-invoice-table.js';
import type { InvoiceTableRow } from './wc-invoice-table.js';
import type { Preview } from '../../preview/types.js';

/** The six seeded invoices, one per derived status. */
const rows: InvoiceTableRow[] = [
  {
    number: 1252,
    status: 'draft',
    clientName: 'Northwind Traders',
    total: 2400,
    balance: 2400,
    dueDate: null,
    href: '#/invoices?number=1252',
  },
  {
    number: 1251,
    status: 'sent',
    clientName: 'Acme Co',
    total: 1850,
    balance: 1850,
    dueDate: '2026-09-06',
    href: '#/invoices?number=1251',
  },
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
    number: 1249,
    status: 'overdue',
    clientName: 'Globex',
    total: 960,
    balance: 960,
    dueDate: '2026-06-30',
    href: '#/invoices?number=1249',
  },
  {
    number: 1248,
    status: 'paid',
    clientName: 'Northwind Traders',
    total: 4000,
    balance: 0,
    dueDate: '2026-07-01',
    href: '#/invoices?number=1248',
  },
  {
    number: 1247,
    status: 'void',
    clientName: 'Globex',
    total: 500,
    balance: null,
    dueDate: null,
    href: '#/invoices?number=1247',
  },
];

const preview: Preview = {
  id: 'wc-invoice-table',
  title: 'Invoice table',
  group: 'Invoicing',
  description:
    'Number, status, client, total, balance and due date. A void invoice shows an em dash for its balance, never an invented $0.00.',
  layout: 'stack',
  states: [
    { name: 'list', render: () => html`<wc-invoice-table .rows=${rows}></wc-invoice-table>` },
    {
      name: 'orphaned-client',
      render: () => html`
        <wc-invoice-table
          .rows=${[{ ...rows[2], clientName: null }]}
        ></wc-invoice-table>
      `,
    },
    { name: 'loading', render: () => html`<wc-invoice-table loading></wc-invoice-table>` },
    {
      name: 'empty',
      render: () => html`
        <wc-invoice-table
          .rows=${[]}
          empty-message="No invoices yet — New invoice."
        ></wc-invoice-table>
      `,
    },
    {
      name: 'unlinked',
      render: () => html`
        <wc-invoice-table
          .rows=${rows.map((row) => ({ ...row, href: undefined }))}
        ></wc-invoice-table>
      `,
    },
  ],
};

export default preview;

import { html } from 'lit';
import './wc-register-table.js';
import type { CategoryOption, RegisterTableRow } from './wc-register-table.js';
import type { Preview } from '../../preview/types.js';

const rows: RegisterTableRow[] = [
  {
    id: 101,
    date: '2025-03-03',
    description: 'ADOBE CREATIVE CLOUD SUBSCRIPTION',
    amount: -54.99,
    category: 'Software / Subscriptions',
    categoryId: 12,
    vendor: 'Adobe',
    accountName: 'BofA Credit Card',
    isFlagged: false,
  },
  {
    id: 102,
    date: '2025-03-05',
    description: 'CLIENT PAYMENT — NORTHWIND',
    amount: 8400,
    category: 'Consulting income',
    categoryId: 3,
    vendor: 'Northwind',
    accountName: 'BofA Checking',
    isFlagged: false,
  },
  {
    id: 103,
    date: '2025-03-07',
    description: 'SQ *BLUE BOTTLE COFFEE',
    amount: -18.5,
    category: null,
    categoryId: null,
    vendor: null,
    accountName: 'BofA Credit Card',
    isFlagged: true,
  },
  {
    id: 104,
    date: '2025-03-11',
    description:
      'WIRE TRANSFER FEE — INTERNATIONAL OUTGOING, REFERENCE 88213-A, DESCRIPTION LONG ENOUGH TO WRAP ACROSS LINES',
    amount: -45,
    category: 'Bank fees',
    categoryId: 21,
    vendor: null,
    accountName: 'BofA Checking',
    isFlagged: false,
  },
];

const categories: CategoryOption[] = [
  { id: 3, name: 'Consulting income', categoryType: 'income' },
  { id: 12, name: 'Software / Subscriptions', categoryType: 'expense' },
  { id: 21, name: 'Bank fees', categoryType: 'expense' },
  { id: 30, name: 'Meals', categoryType: 'expense' },
];

const preview: Preview = {
  id: 'wc-register-table',
  title: 'Register table',
  group: 'Data',
  layout: 'stack',
  description:
    'The transaction register. Selection follows focus through a roving tabindex; a row not being edited renders no wa-* components, because an unfiltered register is thousands of rows.',
  states: [
    {
      name: 'default',
      render: () => html`
        <wc-register-table
          .rows=${rows}
          .categories=${categories}
          .selectedId=${102}
          .total=${8281.51}
          footer-note="4 of 480 rows"
        ></wc-register-table>
      `,
    },
    {
      name: 'dense',
      render: () => html`
        <wc-register-table dense .rows=${rows} .categories=${categories}></wc-register-table>
      `,
    },
    {
      name: 'empty',
      render: () => html`
        <wc-register-table
          .rows=${[]}
          empty-message="No transactions match this search."
        ></wc-register-table>
      `,
    },
    {
      name: 'editing',
      render: () => html`
        <wc-register-table
          .rows=${rows}
          .categories=${categories}
          .selectedId=${103}
          .editingId=${103}
        ></wc-register-table>
      `,
    },
    {
      name: 'flagged',
      render: () => html`
        <wc-register-table
          .rows=${rows.filter((row) => row.isFlagged)}
          .categories=${categories}
        ></wc-register-table>
      `,
    },
    {
      name: 'saving',
      render: () => html`
        <wc-register-table
          .rows=${rows}
          .categories=${categories}
          .selectedId=${101}
          .busyId=${101}
        ></wc-register-table>
      `,
    },
    {
      name: 'no-account-column',
      render: () => html`
        <wc-register-table
          .rows=${rows}
          .categories=${categories}
          .showAccount=${false}
          .total=${-120.99}
        ></wc-register-table>
      `,
    },
    {
      name: 'readonly',
      render: () => html`
        <wc-register-table
          readonly
          .rows=${rows}
          .total=${-120.99}
          footer-note="Read-only — editing lives in the register browser"
        ></wc-register-table>
      `,
    },
  ],
};

export default preview;

import { html } from 'lit';
import './wc-report-table.js';
import type { Preview } from '../../preview/types.js';
import type { ReportColumn, ReportTableRow } from './wc-report-table.js';

const amountColumns: ReportColumn[] = [
  { key: 'name', label: 'Category', kind: 'text' },
  { key: 'amount', label: 'Amount', kind: 'moneyAbs' },
];

/** The P&L amount column is signed; its expense band prints magnitudes. */
const pnlColumns: ReportColumn[] = [
  { key: 'name', label: 'Category', kind: 'text' },
  { key: 'amount', label: 'Amount', kind: 'money' },
];

const magnitude = { amount: 'moneyAbs' } as const;

const pnlRows: ReportTableRow[] = [
  { cells: { name: 'Income' }, emphasis: 'section' },
  { cells: { name: 'Client Services', amount: 8700 }, indent: 1 },
  { cells: { name: 'Interest', amount: 42.15 }, indent: 1 },
  { cells: { name: 'Total Income', amount: 8742.15 }, emphasis: 'subtotal' },
  { cells: { name: 'Expenses' }, emphasis: 'section' },
  {
    cells: { name: 'Software & Subscriptions', amount: -169.97 },
    cellKinds: magnitude,
    indent: 1,
  },
  { cells: { name: 'Bank & Merchant Fees', amount: -24 }, cellKinds: magnitude, indent: 1 },
  {
    cells: { name: 'Total Expenses', amount: -193.97 },
    cellKinds: magnitude,
    emphasis: 'subtotal',
  },
];

const netRow: ReportTableRow = {
  cells: { name: 'Net', amount: 8548.18 },
  emphasis: 'total',
  tone: 'income',
};

const expenseColumns: ReportColumn[] = [
  { key: 'name', label: 'Category', kind: 'text' },
  { key: 'amount', label: 'Amount', kind: 'moneyAbs' },
  { key: 'pct', label: '%', kind: 'percent' },
  { key: 'count', label: 'Count', kind: 'count' },
];

const cashflowColumns: ReportColumn[] = [
  { key: 'month', label: 'Month', kind: 'text' },
  { key: 'inflows', label: 'Inflows', kind: 'moneyAbs' },
  { key: 'outflows', label: 'Outflows', kind: 'moneyAbs' },
  { key: 'net', label: 'Net', kind: 'money' },
  { key: 'running', label: 'Running', kind: 'money' },
];

const flaggedColumns: ReportColumn[] = [
  { key: 'id', label: 'ID', kind: 'count' },
  { key: 'date', label: 'Date', kind: 'text' },
  { key: 'description', label: 'Description', kind: 'text' },
  { key: 'amount', label: 'Amount', kind: 'money' },
  { key: 'account', label: 'Account', kind: 'text' },
];

const preview: Preview = {
  id: 'wc-report-table',
  title: 'Report table',
  group: 'Data',
  layout: 'stack',
  description:
    'One table for every report section. Columns and rows arrive as data, so a report screen maps its response into this shape rather than writing markup — the same way the text reports build comfy_table rows.',
  states: [
    {
      name: 'pnl sections',
      render: () => html`
        <wc-report-table
          caption="Profit and loss"
          .columns=${pnlColumns}
          .rows=${[...pnlRows, netRow]}
        ></wc-report-table>
      `,
    },
    {
      name: 'pnl at a loss',
      render: () => html`
        <wc-report-table
          caption="Profit and loss"
          .columns=${pnlColumns}
          .rows=${[
            ...pnlRows,
            {
              cells: { name: 'Net', amount: -4750 },
              emphasis: 'total',
              tone: 'expense',
            },
          ] satisfies ReportTableRow[]}
        ></wc-report-table>
      `,
    },
    {
      name: 'expenses with percent',
      render: () => html`
        <wc-report-table
          caption="Expense breakdown"
          .columns=${expenseColumns}
          .rows=${[
            { cells: { name: 'Software & Subscriptions', amount: -169.97, pct: 62.4, count: 3 } },
            { cells: { name: 'Bank & Merchant Fees', amount: -24, pct: 8.8, count: 2 } },
            { cells: { name: 'Total', amount: -193.97 }, emphasis: 'total' },
          ] satisfies ReportTableRow[]}
        ></wc-report-table>
      `,
    },
    {
      name: 'cashflow five columns',
      render: () => html`
        <wc-report-table
          caption="Cash flow"
          .columns=${cashflowColumns}
          .rows=${[
            {
              cells: { month: '2025-01', inflows: 5000, outflows: -0, net: 5000, running: 5000 },
            },
            {
              cells: {
                month: '2025-02',
                inflows: 0,
                outflows: -71.99,
                net: -71.99,
                running: 4928.01,
              },
            },
            {
              cells: {
                month: '2025-03',
                inflows: 2500,
                outflows: -300.49,
                net: 2199.51,
                running: 7127.52,
              },
            },
          ] satisfies ReportTableRow[]}
        ></wc-report-table>
      `,
    },
    {
      name: 'k1 summary',
      render: () => html`
        <wc-report-table
          caption="Income summary"
          .columns=${[
            { key: 'item', label: 'Item', kind: 'text' },
            { key: 'amount', label: 'Amount', kind: 'money' },
          ] satisfies ReportColumn[]}
          .rows=${[
            { cells: { item: 'Gross Receipts', amount: 8700 } },
            { cells: { item: 'Cost of Goods Sold', amount: 1200 } },
            { cells: { item: 'Gross Profit', amount: 7500 } },
            { cells: { item: 'Other Income', amount: 0 } },
            { cells: { item: 'Total Deductions', amount: 193.97 } },
            {
              cells: { item: 'Ordinary Business Income', amount: 7306.03 },
              emphasis: 'total',
              tone: 'income',
            },
          ] satisfies ReportTableRow[]}
        ></wc-report-table>
      `,
    },
    {
      name: 'k1 needs mapping',
      render: () => html`
        <wc-report-table
          caption="Needs mapping"
          .columns=${amountColumns}
          .rows=${[
            { cells: { name: 'Mystery Spend', amount: -42 } },
            { cells: { name: 'Studio Sundries', amount: -118.4 } },
          ] satisfies ReportTableRow[]}
        ></wc-report-table>
      `,
    },
    {
      name: 'other deductions with note',
      render: () => html`
        <wc-report-table
          caption="Line 19 — other deductions"
          .columns=${[
            { key: 'name', label: 'Category', kind: 'text' },
            { key: 'total', label: 'Full amount', kind: 'moneyAbs' },
            { key: 'deductible', label: 'Deductible', kind: 'moneyAbs' },
          ] satisfies ReportColumn[]}
          .rows=${[
            { cells: { name: 'Meals', total: 400, deductible: 200 }, note: '(50%)' },
            { cells: { name: 'Dues & Subscriptions', total: 169.97, deductible: 169.97 } },
            {
              cells: { name: 'Total Other Deductions', deductible: 369.97 },
              emphasis: 'total',
            },
          ] satisfies ReportTableRow[]}
        ></wc-report-table>
      `,
    },
    {
      name: 'linked rows',
      render: () => html`
        <wc-report-table
          caption="Flagged transactions"
          .columns=${flaggedColumns}
          .rows=${[
            {
              cells: {
                id: 6,
                date: '2025-03-22',
                description: 'UNKNOWN VENDOR 8812',
                amount: -240.5,
                account: 'BofA Credit Card',
              },
              href: '#/review?id=6',
            },
            {
              cells: {
                id: 9,
                date: '2025-04-02',
                description: 'SQ *COFFEE',
                amount: -7.25,
                account: 'BofA Credit Card',
              },
              href: '#/review?id=9',
            },
          ] satisfies ReportTableRow[]}
        ></wc-report-table>
      `,
    },
    {
      name: 'dense',
      render: () => html`
        <wc-report-table
          dense
          caption="Profit and loss"
          .columns=${pnlColumns}
          .rows=${pnlRows}
        ></wc-report-table>
      `,
    },
    {
      name: 'caption hidden',
      render: () => html`
        <wc-report-table
          caption-hidden
          caption="Tax summary"
          .columns=${amountColumns}
          .rows=${[{ cells: { name: 'Client Services', amount: 8700 } }] satisfies ReportTableRow[]}
        ></wc-report-table>
      `,
    },
    {
      name: 'loading',
      render: () =>
        html`<wc-report-table loading caption="Profit and loss"></wc-report-table>`,
    },
    {
      name: 'error',
      render: () => html`
        <wc-report-table
          caption="Profit and loss"
          error="Could not reach the nigel server."
        ></wc-report-table>
      `,
    },
    {
      name: 'empty',
      render: () => html`
        <wc-report-table
          caption="Flagged transactions"
          .columns=${flaggedColumns}
          empty-message="No flagged transactions."
        ></wc-report-table>
      `,
    },
  ],
};

export default preview;

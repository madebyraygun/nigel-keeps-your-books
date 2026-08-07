import { html } from 'lit';
import './wc-manager-table.js';
import type { Preview } from '../../preview/types.js';
import type { ManagerAction, ManagerColumn, ManagerRow } from './wc-manager-table.js';

const accountColumns: ManagerColumn[] = [
  { key: 'name', label: 'Name' },
  { key: 'type', label: 'Type' },
  { key: 'institution', label: 'Institution' },
  { key: 'lastFour', label: 'Last four' },
];

const accountRows: ManagerRow[] = [
  {
    id: 1,
    label: 'BofA Checking',
    cells: ['BofA Checking', 'Checking', 'Bank of America', '4821'],
  },
  {
    id: 2,
    label: 'Gusto Payroll',
    cells: ['Gusto Payroll', 'Payroll', null, null],
  },
];

const ruleColumns: ManagerColumn[] = [
  { key: 'pattern', label: 'Pattern', mono: true },
  { key: 'matchType', label: 'Match' },
  { key: 'category', label: 'Category' },
  { key: 'vendor', label: 'Vendor' },
  { key: 'priority', label: 'Priority', align: 'end' },
  { key: 'hits', label: 'Hits', align: 'end' },
];

const ruleRows: ManagerRow[] = [
  {
    id: 7,
    label: 'ADOBE',
    cells: ['ADOBE', 'Contains', 'Software / Subscriptions', 'Adobe', 10, 42],
  },
  {
    id: 8,
    label: 'SQ *',
    cells: ['SQ *', 'Starts with', 'Meals / Entertainment', null, 0, 3],
  },
];

const editDelete: ManagerAction[] = [
  { name: 'edit', label: 'Edit', icon: 'wc-icon-edit' },
  { name: 'delete', label: 'Delete', icon: 'wc-icon-trash', variant: 'danger' },
];

const preview: Preview = {
  id: 'wc-manager-table',
  title: 'Manager table',
  group: 'Data',
  description:
    'The editable list every manager screen is built on. Row actions carry the row name in their labels, so a column of Delete buttons is still navigable by ear.',
  layout: 'stack',
  states: [
    {
      name: 'default',
      render: () => html`
        <wc-manager-table
          caption="Accounts"
          .columns=${accountColumns}
          .rows=${accountRows}
          .actions=${editDelete}
        ></wc-manager-table>
      `,
    },
    {
      name: 'with-null-cells',
      render: () => html`
        <wc-manager-table
          caption="Accounts with missing details"
          .columns=${accountColumns}
          .rows=${[accountRows[1]]}
          .actions=${editDelete}
        ></wc-manager-table>
      `,
    },
    {
      name: 'numeric-columns',
      render: () => html`
        <wc-manager-table
          caption="Categorization rules"
          .columns=${ruleColumns}
          .rows=${ruleRows}
          .actions=${editDelete}
        ></wc-manager-table>
      `,
    },
    {
      name: 'busy-row',
      render: () => html`
        <wc-manager-table
          caption="Accounts"
          .columns=${accountColumns}
          .rows=${accountRows}
          .actions=${editDelete}
          .busyId=${1}
        ></wc-manager-table>
      `,
    },
    {
      name: 'single-action',
      render: () => html`
        <wc-manager-table
          caption="Categorization rules"
          .columns=${ruleColumns}
          .rows=${ruleRows}
          .actions=${[editDelete[1]]}
        ></wc-manager-table>
      `,
    },
    {
      name: 'no-actions',
      render: () => html`
        <wc-manager-table
          caption="Accounts, read only"
          .columns=${accountColumns}
          .rows=${accountRows}
        ></wc-manager-table>
      `,
    },
  ],
};

export default preview;

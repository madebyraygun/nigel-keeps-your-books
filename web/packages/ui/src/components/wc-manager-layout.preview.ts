import { html } from 'lit';
import './wc-manager-layout.js';
import './wc-manager-table.js';
import './wc-manager-dialog.js';
import './wc-account-form.js';
import './wc-empty-state.js';
import type { Preview } from '../../preview/types.js';
import type { ManagerAction, ManagerColumn, ManagerRow } from './wc-manager-table.js';

const columns: ManagerColumn[] = [
  { key: 'name', label: 'Name' },
  { key: 'type', label: 'Type' },
  { key: 'institution', label: 'Institution' },
  { key: 'lastFour', label: 'Last four' },
];

const rows: ManagerRow[] = [
  {
    id: 1,
    label: 'BofA Checking',
    cells: ['BofA Checking', 'Checking', 'Bank of America', '4821'],
  },
  {
    id: 2,
    label: 'BofA Credit Card',
    cells: ['BofA Credit Card', 'Credit card', 'Bank of America', null],
  },
];

const actions: ManagerAction[] = [
  { name: 'edit', label: 'Rename', icon: 'wc-icon-edit' },
  { name: 'delete', label: 'Delete', icon: 'wc-icon-trash', variant: 'danger' },
];

const table = () => html`
  <wc-manager-table
    caption="Accounts"
    .columns=${columns}
    .rows=${rows}
    .actions=${actions}
  ></wc-manager-table>
`;

const preview: Preview = {
  id: 'wc-manager-layout',
  title: 'Manager layout',
  group: 'Layout',
  description:
    'The frame the accounts, categories and rules screens share: heading, Add, the list, and the one place a refused delete lands.',
  layout: 'stack',
  states: [
    {
      name: 'list',
      render: () => html`
        <wc-manager-layout heading="Accounts" .count=${2} add-label="Add account">
          ${table()}
        </wc-manager-layout>
      `,
    },
    {
      name: 'empty',
      render: () => html`
        <wc-manager-layout heading="Categorization rules" .count=${0} empty add-label="Add rule">
          <wc-empty-state
            slot="empty"
            icon="wc-icon-rule"
            heading="No rules yet"
            message="Rules categorize future imports automatically. Create one here, or from the review screen."
          ></wc-empty-state>
        </wc-manager-layout>
      `,
    },
    {
      name: 'dialog-open',
      render: () => html`
        <wc-manager-layout heading="Accounts" .count=${2} add-label="Add account">
          ${table()}
          <wc-manager-dialog slot="overlay" open heading="Add account" confirm-label="Add">
            <wc-account-form
              .value=${{
                name: 'Chase Business',
                accountType: 'checking',
                institution: 'Chase',
                lastFour: '9921',
              }}
            ></wc-account-form>
          </wc-manager-dialog>
        </wc-manager-layout>
      `,
    },
    {
      name: 'guardrail-error',
      render: () => html`
        <wc-manager-layout
          heading="Categories"
          .count=${2}
          add-label="Add category"
          error="3 active rules assign this category. Delete those rules first."
          error-action-label="Show those rules"
        >
          ${table()}
        </wc-manager-layout>
      `,
    },
    {
      name: 'busy',
      render: () => html`
        <wc-manager-layout heading="Accounts" busy add-label="Add account">
          ${table()}
        </wc-manager-layout>
      `,
    },
  ],
};

export default preview;

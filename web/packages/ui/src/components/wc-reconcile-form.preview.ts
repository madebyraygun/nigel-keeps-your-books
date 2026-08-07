import { html } from 'lit';
import './wc-reconcile-form.js';
import { EMPTY_RECONCILE_FORM } from './wc-reconcile-form.js';
import type { Preview } from '../../preview/types.js';

const ACCOUNTS = ['BofA Checking', 'BofA Credit Card', 'Line of Credit'];

const filled = {
  account: 'BofA Checking',
  month: '2025-02',
  balance: '4,928.01',
};

const preview: Preview = {
  id: 'wc-reconcile-form',
  title: 'Reconcile form',
  group: 'Data',
  description:
    'Account, month and statement balance. The balance field is the app’s only currency input: a rendered $ prefix, tabular digits, and commas accepted the way the TUI accepts them.',
  layout: 'stack',
  states: [
    {
      name: 'empty',
      render: () => html`
        <wc-reconcile-form
          .accounts=${ACCOUNTS}
          .value=${EMPTY_RECONCILE_FORM}
        ></wc-reconcile-form>
      `,
    },
    {
      name: 'filled',
      render: () => html`
        <wc-reconcile-form .accounts=${ACCOUNTS} .value=${filled}></wc-reconcile-form>
      `,
    },
    {
      name: 'with-errors',
      render: () => html`
        <wc-reconcile-form
          .accounts=${ACCOUNTS}
          .value=${{ ...filled, month: '2025-07', balance: '' }}
          .errors=${{
            month: 'No transactions for that account in that month.',
            balance: 'Balance is required',
          }}
        ></wc-reconcile-form>
      `,
    },
    {
      name: 'busy',
      render: () => html`
        <wc-reconcile-form
          .accounts=${ACCOUNTS}
          .value=${filled}
          busy
        ></wc-reconcile-form>
      `,
    },
    {
      name: 'no-accounts',
      render: () => html`
        <wc-reconcile-form
          .accounts=${[]}
          .value=${EMPTY_RECONCILE_FORM}
        ></wc-reconcile-form>
      `,
    },
  ],
};

export default preview;

import { html } from 'lit';
import './wc-balance-list.js';
import type { BalanceRow } from './wc-balance-list.js';
import type { Preview } from '../../preview/types.js';

const accounts: BalanceRow[] = [
  { name: 'BofA Checking', accountType: 'checking', balance: 42918.44 },
  { name: 'BofA Credit Card', accountType: 'credit_card', balance: -3184.9 },
  { name: 'Line of Credit', accountType: 'line_of_credit', balance: -12000 },
  { name: 'Savings', accountType: 'savings', balance: 60000 },
];

const total = accounts.reduce((sum, a) => sum + a.balance, 0);

const preview: Preview = {
  id: 'wc-balance-list',
  title: 'Balance list',
  group: 'Data',
  description:
    'Account balances as a real table, so the amount column keeps its header association. The totals row appears only when a total is supplied.',
  layout: 'stack',
  states: [
    {
      name: 'populated',
      render: () =>
        html`<wc-balance-list
          .items=${accounts}
          .total=${total}
        ></wc-balance-list>`,
    },
    {
      name: 'without total',
      render: () => html`<wc-balance-list .items=${accounts}></wc-balance-list>`,
    },
    {
      name: 'single account',
      render: () =>
        html`<wc-balance-list .items=${accounts.slice(0, 1)}></wc-balance-list>`,
    },
    {
      name: 'all negative',
      render: () =>
        html`<wc-balance-list
          .items=${accounts.slice(1, 3)}
          .total=${-15184.9}
        ></wc-balance-list>`,
    },
    { name: 'empty', render: () => html`<wc-balance-list></wc-balance-list>` },
    { name: 'loading', render: () => html`<wc-balance-list loading></wc-balance-list>` },
    {
      name: 'error',
      render: () =>
        html`<wc-balance-list
          error="Could not reach the nigel server."
        ></wc-balance-list>`,
    },
  ],
};

export default preview;

import { html } from 'lit';
import './wc-reconciliation-history.js';
import type { ReconciliationHistoryRow } from './wc-reconciliation-history.js';
import type { Preview } from '../../preview/types.js';

const ROWS: ReconciliationHistoryRow[] = [
  {
    id: 4,
    month: '2025-03',
    statementBalance: 5000,
    calculatedBalance: 4871.44,
    isReconciled: false,
    reconciledAt: null,
  },
  {
    id: 3,
    month: '2025-02',
    statementBalance: 4928.01,
    calculatedBalance: 4928.01,
    isReconciled: true,
    reconciledAt: '2025-03-01 12:04:18',
  },
  {
    id: 1,
    month: '2024-12',
    statementBalance: null,
    calculatedBalance: null,
    isReconciled: true,
    reconciledAt: '2025-01-04 10:00:00',
  },
];

const preview: Preview = {
  id: 'wc-reconciliation-history',
  title: 'Reconciliation history',
  group: 'Data',
  description:
    'Which months have been checked and how they came out — mismatches included, because the server records those on purpose. A record with no stored balance shows an em dash, never $0.00.',
  layout: 'stack',
  states: [
    {
      name: 'populated',
      render: () =>
        html`<wc-reconciliation-history .rows=${ROWS}></wc-reconciliation-history>`,
    },
    {
      name: 'empty',
      render: () =>
        html`<wc-reconciliation-history .rows=${[]}></wc-reconciliation-history>`,
    },
    {
      name: 'loading',
      render: () => html`<wc-reconciliation-history loading></wc-reconciliation-history>`,
    },
    {
      name: 'error',
      render: () => html`
        <wc-reconciliation-history
          .error=${'Could not load past reconciliations.'}
        ></wc-reconciliation-history>
      `,
    },
  ],
};

export default preview;

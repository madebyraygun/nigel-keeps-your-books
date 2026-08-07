import { html } from 'lit';
import './wc-import-history.js';
import type { ImportHistoryRow } from './wc-import-history.js';
import type { Preview } from '../../preview/types.js';

const IMPORTS: ImportHistoryRow[] = [
  {
    id: 12,
    filename: 'march-checking.csv',
    accountName: 'BofA Checking',
    importDate: '2025-04-02 09:14:11',
    transactionCount: 42,
  },
  {
    id: 11,
    filename: 'february-card.csv',
    accountName: 'BofA Credit Card',
    importDate: '2025-03-03 17:40:02',
    transactionCount: 17,
  },
  {
    id: 9,
    filename: 'january-checking.csv',
    accountName: 'BofA Checking',
    importDate: '2025-02-01 08:02:55',
    transactionCount: 0,
  },
];

const preview: Preview = {
  id: 'wc-import-history',
  title: 'Import history',
  group: 'Data',
  description:
    'Every import, newest first, each with an Undo. Supersets the TUI, which can only offer the most recent one. An import whose rows are gone still lists, at zero.',
  layout: 'stack',
  states: [
    {
      name: 'populated',
      render: () => html`<wc-import-history .imports=${IMPORTS}></wc-import-history>`,
    },
    {
      name: 'row-busy',
      render: () => html`
        <wc-import-history .imports=${IMPORTS} .busyId=${12}></wc-import-history>
      `,
    },
    {
      name: 'empty',
      render: () => html`<wc-import-history .imports=${[]}></wc-import-history>`,
    },
    {
      name: 'loading',
      render: () => html`<wc-import-history loading></wc-import-history>`,
    },
    {
      name: 'error',
      render: () => html`
        <wc-import-history
          .error=${'Could not load the import history.'}
        ></wc-import-history>
      `,
    },
  ],
};

export default preview;

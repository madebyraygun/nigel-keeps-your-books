import { html } from 'lit';
import './wc-sample-table.js';
import type { Preview } from '../../preview/types.js';
import type { SampleTableRow } from './wc-sample-table.js';

const ROWS: SampleTableRow[] = [
  { date: '2025-04-01', description: 'ACME CORP PAYMENT', amount: 3000 },
  { date: '2025-04-03', description: 'ADOBE CREATIVE CLOUD', amount: -59.99 },
  { date: '2025-04-07', description: 'DIGITALOCEAN.COM', amount: -24 },
  { date: '2025-04-11', description: 'TRANSFER TO SAVINGS', amount: -1500 },
  { date: '2025-04-19', description: 'CONSULTING RETAINER', amount: 4500 },
];

const preview: Preview = {
  id: 'wc-sample-table',
  title: 'Sample Table',
  group: 'Data',
  description: 'Read-only preview of rows parsed from a statement, before import.',
  layout: 'stack',
  states: [
    {
      name: 'default',
      render: () =>
        html`<wc-sample-table
          .rows=${ROWS}
          caption="First 5 rows of april-2025.csv"
        ></wc-sample-table>`,
    },
    {
      name: 'dense',
      render: () => html`<wc-sample-table dense .rows=${ROWS}></wc-sample-table>`,
    },
    {
      name: 'empty',
      render: () =>
        html`<wc-sample-table
          empty-message="Nothing in this file could be parsed."
        ></wc-sample-table>`,
    },
    {
      name: 'long-descriptions',
      render: () =>
        html`<wc-sample-table
          .rows=${[
            {
              date: '2025-04-02',
              description:
                'CHECKCARD 0401 SQ *THE VERY LONG COFFEE COMPANY NAME THAT WRAPS SAN FRANCISCO CA 24445001234567890123456',
              amount: -6.75,
            },
          ]}
        ></wc-sample-table>`,
    },
  ],
};

export default preview;

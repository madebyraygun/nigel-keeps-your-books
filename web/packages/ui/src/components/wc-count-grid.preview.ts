import { html } from 'lit';
import './wc-count-grid.js';
import type { Preview } from '../../preview/types.js';
import type { CountItem } from './wc-count-grid.js';

const PREVIEW_COUNTS: CountItem[] = [
  { label: 'To import', value: 42 },
  { label: 'Duplicates', value: 3 },
  { label: 'Malformed', value: 1, emphasis: 'warn' },
];

const RESULT_COUNTS: CountItem[] = [
  { label: 'Imported', value: 42, emphasis: 'good' },
  { label: 'Duplicates', value: 3 },
  { label: 'Malformed', value: 0 },
  { label: 'Categorized', value: 38, emphasis: 'good' },
  { label: 'Still flagged', value: 6, emphasis: 'warn' },
];

const preview: Preview = {
  id: 'wc-count-grid',
  title: 'Count Grid',
  group: 'Data',
  description: 'Labelled whole numbers — import counts, reconcile tallies.',
  layout: 'stack',
  states: [
    {
      name: 'default',
      render: () => html`<wc-count-grid .items=${PREVIEW_COUNTS}></wc-count-grid>`,
    },
    {
      name: 'result',
      render: () => html`<wc-count-grid .items=${RESULT_COUNTS}></wc-count-grid>`,
    },
    {
      name: 'dense',
      render: () => html`<wc-count-grid dense .items=${RESULT_COUNTS}></wc-count-grid>`,
    },
    {
      name: 'zeroes',
      render: () =>
        html`<wc-count-grid
          .items=${[
            { label: 'Imported', value: 0 },
            { label: 'Duplicates', value: 0 },
          ]}
        ></wc-count-grid>`,
    },
    {
      name: 'with-hint',
      render: () =>
        html`<wc-count-grid
          .items=${[{ label: 'Still flagged', value: 6, hint: 'across the ledger' }]}
        ></wc-count-grid>`,
    },
  ],
};

export default preview;

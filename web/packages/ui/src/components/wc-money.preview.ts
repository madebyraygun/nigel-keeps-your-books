import { html } from 'lit';
import './wc-money.js';
import type { Preview } from '../../preview/types.js';

const rows = [1234.56, -500, 0, 1000000.99, -42.1];

const preview: Preview = {
  id: 'wc-money',
  title: 'Money',
  group: 'Data',
  description:
    'Signed cash amount in the mono stack with tabular figures. Income green, expense red, and the sign always rendered so color is not the only cue.',
  states: [
    { name: 'positive', render: () => html`<wc-money .amount=${1234.56}></wc-money>` },
    { name: 'negative', render: () => html`<wc-money .amount=${-500}></wc-money>` },
    { name: 'zero', render: () => html`<wc-money .amount=${0}></wc-money>` },
    {
      name: 'large',
      render: () => html`<wc-money .amount=${1000000.99}></wc-money>`,
    },
    {
      name: 'plain',
      render: () => html`<wc-money .amount=${-42.1} variant="plain"></wc-money>`,
    },
    {
      name: 'column',
      render: () => html`
        <div style="display:grid;gap:4px;width:180px;">
          ${rows.map(
            (n) => html`<wc-money .amount=${n} align="end"></wc-money>`,
          )}
        </div>
      `,
    },
  ],
};

export default preview;

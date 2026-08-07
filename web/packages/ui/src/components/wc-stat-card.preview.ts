import { html } from 'lit';
import './wc-stat-card.js';
import type { Preview } from '../../preview/types.js';

const row = html`
  <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:12px;">
    <wc-stat-card label="YTD Income" .amount=${184320.5}></wc-stat-card>
    <wc-stat-card label="YTD Expenses" .amount=${-96540.18}></wc-stat-card>
    <wc-stat-card label="Net" .amount=${87780.32}></wc-stat-card>
  </div>
`;

const preview: Preview = {
  id: 'wc-stat-card',
  title: 'Stat card',
  group: 'Data',
  description:
    'One headline number with its label. Loading and error live on the card, not the screen, because each dashboard figure is fetched on its own.',
  states: [
    {
      name: 'positive',
      render: () =>
        html`<wc-stat-card label="YTD Income" .amount=${184320.5}></wc-stat-card>`,
    },
    {
      name: 'negative',
      render: () =>
        html`<wc-stat-card
          label="YTD Expenses"
          .amount=${-96540.18}
        ></wc-stat-card>`,
    },
    {
      name: 'zero',
      render: () => html`<wc-stat-card label="Net" .amount=${0}></wc-stat-card>`,
    },
    {
      name: 'with hint',
      render: () =>
        html`<wc-stat-card
          label="Net"
          .amount=${87780.32}
          hint="Year to date, cash basis"
        ></wc-stat-card>`,
    },
    {
      name: 'loading',
      render: () => html`<wc-stat-card label="YTD Income" loading></wc-stat-card>`,
    },
    {
      name: 'error',
      render: () =>
        html`<wc-stat-card
          label="YTD Income"
          error="Could not reach the nigel server."
        ></wc-stat-card>`,
    },
    { name: 'row of three', render: () => row },
  ],
};

export default preview;

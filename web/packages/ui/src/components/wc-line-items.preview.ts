import { html } from 'lit';
import './wc-line-items.js';
import type { LineItemValue } from './wc-line-items.js';
import type { Preview } from '../../preview/types.js';

const one: LineItemValue[] = [
  { description: 'Consulting — August', quantity: '10', unitAmount: '150' },
];

const many: LineItemValue[] = [
  { description: 'Consulting — August', quantity: '10', unitAmount: '150' },
  { description: 'Hosting', quantity: '1', unitAmount: '350' },
  { description: 'Out-of-hours support', quantity: '2.5', unitAmount: '220' },
];

const preview: Preview = {
  id: 'wc-line-items',
  title: 'Line items',
  group: 'Invoicing',
  description:
    'The repeatable rows an invoice is built from, with a live subtotal. Reordering is up/down buttons — a drag handle has no keyboard equivalent that passes axe without building these anyway.',
  layout: 'stack',
  states: [
    { name: 'one-row', render: () => html`<wc-line-items .items=${one}></wc-line-items>` },
    { name: 'many-rows', render: () => html`<wc-line-items .items=${many}></wc-line-items>` },
    {
      name: 'empty',
      render: () => html`
        <wc-line-items
          .items=${[]}
          list-error="An invoice needs at least one line."
        ></wc-line-items>
      `,
    },
    {
      name: 'readonly',
      render: () => html`
        <wc-line-items readonly .items=${many} .total=${2400}></wc-line-items>
      `,
    },
    {
      name: 'field-errors',
      render: () => html`
        <wc-line-items
          .items=${[
            { description: '', quantity: 'lots', unitAmount: '' },
            ...one,
          ]}
          .errors=${[
            {
              description: 'Description is required',
              quantity: 'Quantity must be a number',
              unitAmount: 'Unit amount is required',
            },
          ]}
        ></wc-line-items>
      `,
    },
    {
      name: 'saving',
      render: () => html`<wc-line-items .items=${many} disabled></wc-line-items>`,
    },
  ],
};

export default preview;

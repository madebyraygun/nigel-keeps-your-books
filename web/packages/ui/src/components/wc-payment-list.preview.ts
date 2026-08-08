import { html } from 'lit';
import './wc-payment-list.js';
import type { PaymentRow } from './wc-payment-list.js';
import type { Preview } from '../../preview/types.js';

const manual: PaymentRow = {
  id: 1,
  amount: 2000,
  paidDate: '2026-03-02',
  method: 'direct_deposit',
  stripeCheckoutSessionId: null,
};

const stripe: PaymentRow = {
  id: 2,
  amount: 1200,
  paidDate: '2026-03-09',
  method: 'stripe',
  stripeCheckoutSessionId: 'cs_test_a1b2c3',
};

const ach: PaymentRow = {
  id: 3,
  amount: 500,
  paidDate: '2026-03-11',
  method: 'ach',
  stripeCheckoutSessionId: null,
};

const preview: Preview = {
  id: 'wc-payment-list',
  title: 'Payment list',
  group: 'Invoicing',
  description:
    'An invoice’s payment history. A payment pulled from Stripe is marked, because it is the one nobody typed.',
  layout: 'stack',
  states: [
    { name: 'empty', render: () => html`<wc-payment-list .payments=${[]}></wc-payment-list>` },
    { name: 'one', render: () => html`<wc-payment-list .payments=${[manual]}></wc-payment-list>` },
    {
      name: 'many',
      render: () =>
        html`<wc-payment-list .payments=${[manual, stripe, ach]}></wc-payment-list>`,
    },
    {
      name: 'stripe-only',
      render: () => html`<wc-payment-list .payments=${[stripe]}></wc-payment-list>`,
    },
  ],
};

export default preview;

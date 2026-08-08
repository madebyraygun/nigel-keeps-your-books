import { html } from 'lit';
import './wc-payment-form.js';
import { paymentFormFor, PAYMENT_METHOD_VALUES } from './wc-payment-form.js';
import type { Preview } from '../../preview/types.js';

const full = paymentFormFor(1200, '2026-03-15');

const preview: Preview = {
  id: 'wc-payment-form',
  title: 'Payment form',
  group: 'Invoicing',
  description:
    'Amount, date and method. The amount defaults to the whole outstanding balance and reuses the reconcile form’s currency treatment.',
  layout: 'stack',
  states: [
    {
      name: 'full-balance',
      render: () => html`<wc-payment-form .value=${full} .balance=${1200}></wc-payment-form>`,
    },
    {
      name: 'partial',
      render: () => html`
        <wc-payment-form
          .value=${{ ...full, amount: '500.00' }}
          .balance=${1200}
        ></wc-payment-form>
      `,
    },
    {
      name: 'whole-balance-by-default',
      render: () => html`
        <wc-payment-form .value=${{ ...full, amount: '' }} .balance=${1200}></wc-payment-form>
      `,
    },
    ...PAYMENT_METHOD_VALUES.map((method) => ({
      name: `method-${method}`,
      render: () => html`
        <wc-payment-form .value=${{ ...full, method }} .balance=${1200}></wc-payment-form>
      `,
    })),
    {
      name: 'errors',
      render: () => html`
        <wc-payment-form
          .value=${{ amount: 'lots', date: '2026-3-1', method: 'bitcoin' }}
          .balance=${1200}
          .errors=${{
            amount: 'Invalid payment amount',
            date: 'Date must be YYYY-MM-DD',
            method: 'Method must be one of: direct_deposit, ach, stripe, other',
          }}
        ></wc-payment-form>
      `,
    },
    {
      name: 'saving',
      render: () =>
        html`<wc-payment-form .value=${full} .balance=${1200} disabled></wc-payment-form>`,
    },
  ],
};

export default preview;

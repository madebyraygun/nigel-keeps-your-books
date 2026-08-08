import { describe, it, expect, afterEach } from 'vitest';
import './wc-payment-list.js';
import {
  paymentMethodLabel,
  type PaymentRow,
  type WcPaymentList,
} from './wc-payment-list.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-payment-list.preview.js';

const PAYMENTS: PaymentRow[] = [
  {
    id: 1,
    amount: 2000,
    paidDate: '2026-03-02',
    method: 'direct_deposit',
    stripeCheckoutSessionId: null,
  },
  {
    id: 2,
    amount: 1200,
    paidDate: '2026-03-09',
    method: 'stripe',
    stripeCheckoutSessionId: 'cs_test_a1b2c3',
  },
];

async function mount(props: Partial<WcPaymentList> = {}): Promise<WcPaymentList> {
  const el = document.createElement('wc-payment-list');
  Object.assign(el, { payments: PAYMENTS }, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('paymentMethodLabel', () => {
  it('humanizes the four the CHECK constraint allows', () => {
    expect(paymentMethodLabel('direct_deposit')).toBe('Direct deposit');
    expect(paymentMethodLabel('ach')).toBe('ACH');
    expect(paymentMethodLabel('stripe')).toBe('Stripe');
    expect(paymentMethodLabel('other')).toBe('Other');
  });

  it('passes an unknown method through rather than blanking it', () => {
    expect(paymentMethodLabel('cheque')).toBe('cheque');
  });
});

describe('wc-payment-list', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('lists one row per payment in the order it was given', async () => {
    const el = await mount();
    const dates = [...(el.shadowRoot?.querySelectorAll('tbody tr') ?? [])].map(
      (tr) => tr.querySelector('td')?.textContent?.trim(),
    );
    expect(dates).toEqual(['2026-03-02', '2026-03-09']);
  });

  it('marks a payment that came in through sync', async () => {
    const el = await mount();
    const marks = [...(el.shadowRoot?.querySelectorAll('[data-synced]') ?? [])];
    expect(marks).toHaveLength(1);
    expect(
      el.shadowRoot?.querySelector('tr[data-row="2"]')?.textContent,
    ).toContain('Stripe');
  });

  it('renders each amount through wc-money', async () => {
    const el = await mount();
    const amounts = [...(el.shadowRoot?.querySelectorAll('wc-money') ?? [])].map(
      (money) => (money as HTMLElement & { amount: number }).amount,
    );
    expect(amounts).toEqual([2000, 1200]);
  });

  it('says there are none rather than rendering an empty table', async () => {
    const el = await mount({ payments: [] });
    expect(el.shadowRoot?.querySelector('table')).toBeNull();
    expect(el.shadowRoot?.querySelector('[data-empty]')?.textContent).toContain(
      'No payments recorded',
    );
  });
});

describePreviewA11y(preview);

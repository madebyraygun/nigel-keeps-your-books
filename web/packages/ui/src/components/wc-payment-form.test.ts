import { describe, it, expect, afterEach } from 'vitest';
import './wc-payment-form.js';
import {
  EMPTY_PAYMENT_FORM,
  PAYMENT_METHOD_VALUES,
  paymentFormFor,
  validatePaymentForm,
  type NcPaymentFormChangeDetail,
  type PaymentFormValue,
  type WcPaymentForm,
} from './wc-payment-form.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-payment-form.preview.js';

const valid: PaymentFormValue = {
  amount: '1,200.00',
  date: '2026-03-15',
  method: 'direct_deposit',
};

async function mount(props: Partial<WcPaymentForm> = {}): Promise<WcPaymentForm> {
  const el = document.createElement('wc-payment-form');
  Object.assign(el, { value: valid, balance: 1200 }, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function changes(el: WcPaymentForm): PaymentFormValue[] {
  const seen: PaymentFormValue[] = [];
  el.addEventListener('nc-payment-form-change', (event) =>
    seen.push((event as CustomEvent<NcPaymentFormChangeDetail>).detail.value),
  );
  return seen;
}

describe('payment form vocabulary', () => {
  it('offers exactly the methods the CHECK constraint allows', () => {
    expect([...PAYMENT_METHOD_VALUES]).toEqual([
      'direct_deposit',
      'ach',
      'stripe',
      'other',
    ]);
  });

  it('defaults to direct deposit, as the pay route does', () => {
    expect(EMPTY_PAYMENT_FORM.method).toBe('direct_deposit');
  });

  it('seeds the amount with the whole balance and the date with today', () => {
    expect(paymentFormFor(1200.5, '2026-03-15')).toEqual({
      amount: '1,200.50',
      date: '2026-03-15',
      method: 'direct_deposit',
    });
  });
});

describe('validatePaymentForm', () => {
  it('accepts a well-formed payment', () => {
    expect(validatePaymentForm(valid)).toEqual({});
  });

  it('treats an empty amount as the whole balance rather than an error', () => {
    expect(validatePaymentForm({ ...valid, amount: '' })).toEqual({});
  });

  it('refuses an unreadable, zero or negative amount', () => {
    // `payment_amount`'s own rules: a NaN compares false against every bound
    // and poisons every later SUM, so it is refused where it can still be
    // corrected as well as at the route.
    expect(validatePaymentForm({ ...valid, amount: 'lots' }).amount).toBe(
      'Invalid payment amount',
    );
    expect(validatePaymentForm({ ...valid, amount: '0' }).amount).toContain(
      'greater than zero',
    );
    expect(validatePaymentForm({ ...valid, amount: '-5' }).amount).toContain(
      'greater than zero',
    );
  });

  it('requires a zero-padded date, as the API does', () => {
    expect(validatePaymentForm({ ...valid, date: '' }).date).toContain('required');
    expect(validatePaymentForm({ ...valid, date: '2026-3-1' }).date).toBe(
      'Date must be YYYY-MM-DD',
    );
  });

  it('names the legal set for an unknown method', () => {
    expect(validatePaymentForm({ ...valid, method: 'bitcoin' }).method).toContain(
      'direct_deposit',
    );
  });
});

describe('wc-payment-form', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('collects an amount, a date and a method', async () => {
    const el = await mount();
    for (const hook of ['[data-amount]', '[data-date]', '[data-method]']) {
      expect(el.shadowRoot?.querySelector(hook), hook).toBeTruthy();
    }
  });

  it('emits the whole value on every edit', async () => {
    const el = await mount();
    const seen = changes(el);

    const input = el.shadowRoot?.querySelector<HTMLInputElement>('[data-date]');
    input!.value = '2026-04-01';
    input!.dispatchEvent(new Event('input'));

    expect(seen).toEqual([{ ...valid, date: '2026-04-01' }]);
  });

  it('tidies a readable amount on blur and leaves an unreadable one alone', async () => {
    const typed = await mount({ value: { ...valid, amount: '1200.5' } });
    const seenTyped = changes(typed);
    typed.shadowRoot?.querySelector('[data-amount]')?.dispatchEvent(new Event('blur'));
    expect(seenTyped).toEqual([{ ...valid, amount: '1,200.50' }]);

    const junk = await mount({ value: { ...valid, amount: 'lots' } });
    const seenJunk = changes(junk);
    junk.shadowRoot?.querySelector('[data-amount]')?.dispatchEvent(new Event('blur'));
    expect(seenJunk).toEqual([]);
  });

  it('says what an empty amount will record', async () => {
    const el = await mount({ value: { ...valid, amount: '' } });
    expect(el.shadowRoot?.querySelector('[data-amount-hint]')?.textContent).toContain(
      '1,200.00',
    );
  });

  it('renders each error beside its field', async () => {
    const el = await mount({
      errors: { amount: 'Invalid payment amount', date: 'Date must be YYYY-MM-DD' },
    });
    const messages = [...(el.shadowRoot?.querySelectorAll('.error') ?? [])].map((p) =>
      p.textContent?.trim(),
    );
    expect(messages).toEqual(['Invalid payment amount', 'Date must be YYYY-MM-DD']);
  });

  it('disables every control while a save is in flight', async () => {
    const el = await mount({ disabled: true });
    const controls = [
      ...(el.shadowRoot?.querySelectorAll('[data-amount],[data-date],[data-method]') ?? []),
    ];
    expect(controls).toHaveLength(3);
    expect(controls.every((control) => control.hasAttribute('disabled'))).toBe(true);
  });
});

describePreviewA11y(preview);

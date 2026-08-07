import { describe, it, expect, afterEach } from 'vitest';
import './wc-reconcile-form.js';
import {
  EMPTY_RECONCILE_FORM,
  formatStatementBalance,
  parseStatementBalance,
  validateReconcileForm,
  type NcReconcileSubmitDetail,
  type ReconcileFormValue,
  type WcReconcileForm,
} from './wc-reconcile-form.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-reconcile-form.preview.js';

const ACCOUNTS = ['BofA Checking', 'BofA Credit Card'];

const filled: ReconcileFormValue = {
  account: 'BofA Checking',
  month: '2025-02',
  balance: '4928.01',
};

async function mount(props: Partial<WcReconcileForm> = {}): Promise<WcReconcileForm> {
  const el = document.createElement('wc-reconcile-form');
  Object.assign(el, { accounts: ACCOUNTS, value: EMPTY_RECONCILE_FORM }, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

/** Every value the form reports, in order — an array so nothing narrows to never. */
function changes(el: WcReconcileForm): ReconcileFormValue[] {
  const seen: ReconcileFormValue[] = [];
  el.addEventListener('nc-reconcile-change', (event) => {
    seen.push((event as CustomEvent<{ value: ReconcileFormValue }>).detail.value);
  });
  return seen;
}

function submit(el: WcReconcileForm): NcReconcileSubmitDetail | null {
  let detail: NcReconcileSubmitDetail | null = null;
  el.addEventListener('nc-reconcile-submit', (event) => {
    detail = (event as CustomEvent<NcReconcileSubmitDetail>).detail;
  });
  el.shadowRoot?.querySelector('form')?.dispatchEvent(
    new Event('submit', { cancelable: true }),
  );
  return detail;
}

afterEach(() => {
  document.body.innerHTML = '';
});

describe('parseStatementBalance', () => {
  it('strips the commas a figure is copied off a statement with', () => {
    // reconcile_manager.rs parses `self.balance.replace(',', "")`.
    expect(parseStatementBalance('4,928.01')).toBe(4928.01);
    expect(parseStatementBalance('1,234,567.89')).toBe(1234567.89);
  });

  it('takes a plain number and a negative one', () => {
    expect(parseStatementBalance('4928.01')).toBe(4928.01);
    expect(parseStatementBalance('-250')).toBe(-250);
  });

  it('rejects an empty field and anything that is not a number', () => {
    expect(parseStatementBalance('')).toBeNull();
    expect(parseStatementBalance('   ')).toBeNull();
    expect(parseStatementBalance('twelve')).toBeNull();
    expect(parseStatementBalance('12.3.4')).toBeNull();
    expect(parseStatementBalance('$500')).toBeNull();
  });
});

describe('formatStatementBalance', () => {
  it('groups and pads to cents', () => {
    expect(formatStatementBalance(4928.1, 'en-US')).toBe('4,928.10');
    expect(formatStatementBalance(-250, 'en-US')).toBe('-250.00');
  });

  it('round-trips back through the parser', () => {
    const formatted = formatStatementBalance(1234567.89, 'en-US');
    expect(parseStatementBalance(formatted)).toBe(1234567.89);
  });
});

describe('validateReconcileForm', () => {
  it('uses the TUI’s wording for a missing month and balance', () => {
    const errors = validateReconcileForm(EMPTY_RECONCILE_FORM);
    expect(errors.month).toBe('Month is required (YYYY-MM)');
    expect(errors.balance).toBe('Balance is required');
    expect(errors.account).toBe('Account is required');
  });

  it('rejects a month that is not YYYY-MM', () => {
    expect(validateReconcileForm({ ...filled, month: '2025-13' }).month).toBe(
      'Month must be YYYY-MM',
    );
    expect(validateReconcileForm({ ...filled, month: 'March' }).month).toBe(
      'Month must be YYYY-MM',
    );
    expect(validateReconcileForm({ ...filled, month: '2025-02' }).month).toBeUndefined();
  });

  it('rejects an unparseable balance in the TUI’s words', () => {
    expect(validateReconcileForm({ ...filled, balance: 'lots' }).balance).toBe(
      'Invalid balance amount',
    );
  });

  it('passes a filled form', () => {
    expect(validateReconcileForm(filled)).toEqual({});
  });
});

describe('wc-reconcile-form', () => {
  it('submits the parsed balance, not the typed string', async () => {
    const el = await mount({ value: { ...filled, balance: '4,928.01' } });

    expect(submit(el)).toEqual({
      account: 'BofA Checking',
      month: '2025-02',
      statementBalance: 4928.01,
    });
  });

  it('blocks submit and shows its own errors when the form is incomplete', async () => {
    const el = await mount();

    expect(submit(el)).toBeNull();
    await el.updateComplete;

    const messages = [...(el.shadowRoot?.querySelectorAll('.error') ?? [])].map(
      (node) => node.textContent?.trim(),
    );
    expect(messages).toContain('Month is required (YYYY-MM)');
    expect(messages).toContain('Balance is required');
  });

  it('renders a server error against the field it belongs to', async () => {
    const el = await mount({
      value: filled,
      errors: { month: 'No transactions for that account in that month.' },
    });

    const message = el.shadowRoot?.querySelector('.month')?.parentElement
      ?.querySelector('.error')
      ?.textContent?.trim();
    expect(message).toBe('No transactions for that account in that month.');
  });

  it('reports what the value would become rather than editing it', async () => {
    const el = await mount({ value: filled });
    const reported = changes(el);

    const month = el.shadowRoot?.querySelector('.month') as HTMLElement & {
      value: string;
    };
    month.value = '2025-03';
    month.dispatchEvent(new Event('input'));

    expect(reported.at(-1)).toEqual({ ...filled, month: '2025-03' });
    // Controlled: the property is the screen's to change, not the form's.
    expect(el.value.month).toBe('2025-02');
  });

  it('tidies the balance on blur, and leaves an unparseable one alone', async () => {
    const el = await mount({ value: { ...filled, balance: '4928.1' } });
    const tidied = changes(el);

    el.shadowRoot?.querySelector('.balance')?.dispatchEvent(new Event('blur'));
    expect(tidied.at(-1)?.balance).toBe(formatStatementBalance(4928.1));

    const messy = await mount({ value: { ...filled, balance: 'lots' } });
    const untouched = changes(messy);
    messy.shadowRoot?.querySelector('.balance')?.dispatchEvent(new Event('blur'));
    expect(untouched).toHaveLength(0);
  });

  it('will not submit while busy, or with no account to reconcile', async () => {
    const busy = await mount({ value: filled, busy: true });
    expect(submit(busy)).toBeNull();

    const empty = await mount({ accounts: [], value: filled });
    expect(submit(empty)).toBeNull();
  });

  it('uses a month input, which jsdom and every browser but Safari implement', async () => {
    const el = await mount({ value: filled });
    const month = el.shadowRoot?.querySelector('.month');
    expect(month?.getAttribute('type')).toBe('month');
  });
});

describePreviewA11y(preview);

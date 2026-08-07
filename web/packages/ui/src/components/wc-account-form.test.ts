import { describe, it, expect, afterEach } from 'vitest';
import './wc-account-form.js';
import {
  EMPTY_ACCOUNT_FORM,
  validateAccountForm,
  type AccountFormValue,
  type NcAccountFormChangeDetail,
  type WcAccountForm,
} from './wc-account-form.js';
import { accountTypeLabel, ACCOUNT_TYPES } from './account-type.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-account-form.preview.js';

const filled: AccountFormValue = {
  name: 'BofA Checking',
  accountType: 'checking',
  institution: 'Bank of America',
  lastFour: '4821',
};

async function mount(props: Partial<WcAccountForm> = {}): Promise<WcAccountForm> {
  const el = document.createElement('wc-account-form');
  Object.assign(el, { value: EMPTY_ACCOUNT_FORM }, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('account types', () => {
  it('offers the four the data layer accepts, in the TUI order', () => {
    expect(ACCOUNT_TYPES).toEqual([
      'checking',
      'credit_card',
      'line_of_credit',
      'payroll',
    ]);
  });

  it('humanizes a known type and passes an unknown one through', () => {
    expect(accountTypeLabel('line_of_credit')).toBe('Line of credit');
    expect(accountTypeLabel('brokerage')).toBe('brokerage');
  });
});

describe('validateAccountForm', () => {
  it('requires a name', () => {
    expect(validateAccountForm({ ...filled, name: '   ' }).name).toBe('Name is required');
  });

  it('accepts an empty last four but not a malformed one', () => {
    // The rule lives in account_manager.rs and nowhere else — the route takes
    // any string — so the web has to carry it or be the laxer surface.
    expect(validateAccountForm({ ...filled, lastFour: '' }).lastFour).toBeUndefined();
    expect(validateAccountForm({ ...filled, lastFour: '4821' }).lastFour).toBeUndefined();
    expect(validateAccountForm({ ...filled, lastFour: '12a' }).lastFour).toBe(
      'Last four must be exactly 4 digits',
    );
    expect(validateAccountForm({ ...filled, lastFour: '48210' }).lastFour).toBe(
      'Last four must be exactly 4 digits',
    );
  });
});

describe('wc-account-form', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('collects all four fields in create mode', async () => {
    const el = await mount();
    for (const hook of ['[data-name]', '[data-type]', '[data-institution]', '[data-last-four]']) {
      expect(el.shadowRoot?.querySelector(hook), hook).toBeTruthy();
    }
  });

  it('collects only the name in rename mode, and says why', async () => {
    const el = await mount({ mode: 'rename', value: filled });
    expect(el.shadowRoot?.querySelector('[data-name]')).toBeTruthy();
    expect(el.shadowRoot?.querySelector('[data-type]')).toBeNull();
    expect(el.shadowRoot?.querySelector('[data-institution]')).toBeNull();
    expect(el.shadowRoot?.querySelector('.hint')?.textContent).toContain(
      'set when the account is created',
    );
    expect(el.shadowRoot?.querySelector('.fixed')?.textContent).toContain(
      'Bank of America',
    );
  });

  it('emits the whole value on every edit', async () => {
    const el = await mount({ value: filled });
    const seen: AccountFormValue[] = [];
    el.addEventListener('nc-account-form-change', (event) =>
      seen.push((event as CustomEvent<NcAccountFormChangeDetail>).detail.value),
    );

    const input = el.shadowRoot?.querySelector<HTMLInputElement>('[data-name]');
    input!.value = 'Chase Business';
    input!.dispatchEvent(new Event('input'));

    expect(seen).toEqual([{ ...filled, name: 'Chase Business' }]);
  });

  it('renders field errors beside the fields they belong to', async () => {
    const el = await mount({
      errors: { name: 'Name is required', lastFour: 'Last four must be exactly 4 digits' },
    });
    const messages = [...(el.shadowRoot?.querySelectorAll('.error') ?? [])].map((p) =>
      p.textContent?.trim(),
    );
    expect(messages).toEqual([
      'Name is required',
      'Last four must be exactly 4 digits',
    ]);
  });
});

describePreviewA11y(preview);

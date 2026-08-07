import { describe, it, expect, afterEach } from 'vitest';
import './wc-import-form.js';
import {
  DEFAULT_CSV_MAPPING,
  EMPTY_IMPORT_FORM,
  GENERIC_FORMAT_CHOICE,
  type ImportFormValue,
  type WcImportForm,
} from './wc-import-form.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-import-form.preview.js';

const ACCOUNTS = [
  { id: 1, name: 'BofA Checking', accountType: 'checking' },
  { id: 2, name: 'BofA Credit Card', accountType: 'credit_card' },
];

const FORMATS = [
  { key: 'bofa_checking', name: 'Bank of America Checking', accountTypes: ['checking'] },
];

async function mount(props: Partial<WcImportForm> = {}): Promise<WcImportForm> {
  const el = document.createElement('wc-import-form');
  Object.assign(el, { accounts: ACCOUNTS, formats: FORMATS, ...props });
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

/** The last value the form reported, or null if it never spoke. */
function listen(el: WcImportForm): () => ImportFormValue | null {
  let latest: ImportFormValue | null = null;
  el.addEventListener('nc-import-change', (event) => {
    latest = (event as CustomEvent).detail.value;
  });
  return () => latest;
}

function setValue(el: WcImportForm, selector: string, value: string, event = 'change') {
  const field = el.shadowRoot?.querySelector(selector) as HTMLElement & {
    value: string;
  };
  field.value = value;
  field.dispatchEvent(new Event(event, { bubbles: true, composed: true }));
}

describe('wc-import-form', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('offers detect, every built-in, every profile, and generic', async () => {
    const el = await mount({ profiles: ['chase'] });
    const options = [...(el.shadowRoot?.querySelectorAll('wa-select wa-option') ?? [])];
    const formatOptions = options.filter(
      (option) => !ACCOUNTS.some((a) => a.name === option.getAttribute('value')),
    );

    expect(formatOptions.map((o) => o.getAttribute('value'))).toEqual([
      '',
      'bofa_checking',
      'chase',
      GENERIC_FORMAT_CHOICE,
    ]);
  });

  it('lists every account', async () => {
    const el = await mount();
    const accountSelect = el.shadowRoot?.querySelector('wa-select');
    const options = [...(accountSelect?.querySelectorAll('wa-option') ?? [])];
    expect(options.map((o) => o.getAttribute('value'))).toEqual([
      'BofA Checking',
      'BofA Credit Card',
    ]);
  });

  it('reports the chosen account', async () => {
    const el = await mount();
    const latest = listen(el);

    setValue(el, 'wa-select', 'BofA Credit Card');

    expect(latest()?.account).toBe('BofA Credit Card');
  });

  it('shows the selected account type as a hint', async () => {
    const el = await mount({
      value: { ...EMPTY_IMPORT_FORM, account: 'BofA Credit Card' },
    });
    expect(el.shadowRoot?.querySelector('.hint')?.textContent?.trim()).toBe(
      'credit card',
    );
  });

  it('says so when there are no accounts', async () => {
    const el = await mount({ accounts: [] });
    expect(el.shadowRoot?.querySelector('.hint')?.textContent).toContain(
      'No accounts yet',
    );
  });

  it('keeps the mapping fields hidden until Generic CSV is chosen', async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelector('.mapping')).toBeNull();
  });

  it('reveals the mapping fields and the profile name for Generic CSV', async () => {
    const el = await mount({
      value: { ...EMPTY_IMPORT_FORM, format: GENERIC_FORMAT_CHOICE },
    });

    expect(el.shadowRoot?.querySelector('.mapping')).toBeTruthy();
    for (const field of ['.date-col', '.desc-col', '.amount-col', '.date-format', '.save-profile']) {
      expect(el.shadowRoot?.querySelector(field), field).toBeTruthy();
    }
  });

  it('defaults the mapping to the CLI defaults', async () => {
    expect(DEFAULT_CSV_MAPPING).toEqual({
      dateCol: 0,
      descCol: 1,
      amountCol: 2,
      dateFormat: '%m/%d/%Y',
    });
  });

  it('reports edited column positions as numbers', async () => {
    const el = await mount({
      value: { ...EMPTY_IMPORT_FORM, format: GENERIC_FORMAT_CHOICE },
    });
    const latest = listen(el);

    setValue(el, '.amount-col', '4', 'input');

    expect(latest()?.mapping).toEqual({ ...DEFAULT_CSV_MAPPING, amountCol: 4 });
    expect(typeof latest()?.mapping.amountCol).toBe('number');
  });

  it('ignores a cleared column rather than reporting NaN', async () => {
    const el = await mount({
      value: { ...EMPTY_IMPORT_FORM, format: GENERIC_FORMAT_CHOICE },
    });
    const latest = listen(el);

    setValue(el, '.date-col', '', 'input');

    expect(latest()).toBeNull();
  });

  it('reports the date format and the profile name', async () => {
    const el = await mount({
      value: { ...EMPTY_IMPORT_FORM, format: GENERIC_FORMAT_CHOICE },
    });
    const latest = listen(el);

    setValue(el, '.date-format', '%d/%m/%Y', 'input');
    expect(latest()?.mapping.dateFormat).toBe('%d/%m/%Y');

    setValue(el, '.save-profile', 'chase', 'input');
    expect(latest()?.saveProfile).toBe('chase');
  });

  it('never reports a format and a mapping as separate choices', async () => {
    // The format is one field, so "both" is unrepresentable — this is the
    // structural reason the screen can never send the API a 400-worthy body.
    const el = await mount();
    const latest = listen(el);

    setValue(el, 'wa-select:nth-of-type(1)', 'BofA Checking');
    const value = latest();
    expect(value).not.toBeNull();
    expect(typeof value?.format).toBe('string');
  });

  it('renders each error under its own control', async () => {
    const el = await mount({
      value: { ...EMPTY_IMPORT_FORM, format: GENERIC_FORMAT_CHOICE },
      accountError: 'no such account',
      formatError: 'no such format',
      mappingError: 'bad columns',
    });

    const errors = [...(el.shadowRoot?.querySelectorAll('.error') ?? [])];
    expect(errors.map((e) => e.textContent)).toEqual([
      'no such account',
      'no such format',
      'bad columns',
    ]);
    expect(el.shadowRoot?.querySelector('.mapping .error')?.textContent).toBe(
      'bad columns',
    );
  });

  it('disables every control when disabled', async () => {
    const el = await mount({
      disabled: true,
      value: { ...EMPTY_IMPORT_FORM, format: GENERIC_FORMAT_CHOICE },
    });

    const controls = [
      ...(el.shadowRoot?.querySelectorAll('wa-select, wa-input') ?? []),
    ];
    expect(controls.length).toBeGreaterThan(0);
    expect(controls.every((c) => c.hasAttribute('disabled'))).toBe(true);
  });

  it('does not mutate the value it was given', async () => {
    const value: ImportFormValue = { ...EMPTY_IMPORT_FORM, account: 'BofA Checking' };
    const el = await mount({ value });
    const snapshot = structuredClone(value);
    const latest = listen(el);

    setValue(el, 'wa-select', 'BofA Credit Card');

    expect(value).toEqual(snapshot);
    expect(latest()?.account).toBe('BofA Credit Card');
  });
});

describePreviewA11y(preview);

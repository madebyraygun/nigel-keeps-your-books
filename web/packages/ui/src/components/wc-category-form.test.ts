import { describe, it, expect, afterEach } from 'vitest';
import './wc-category-form.js';
import {
  EMPTY_CATEGORY_FORM,
  formLineSuggestions,
  formLineWarning,
  validateCategoryForm,
  type CategoryFormValue,
  type NcCategoryFormChangeDetail,
  type WcCategoryForm,
} from './wc-category-form.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-category-form.preview.js';

const filled: CategoryFormValue = {
  name: 'Software / Subscriptions',
  categoryType: 'expense',
  taxLine: 'Other expenses',
  formLine: '1120S-19',
};

async function mount(props: Partial<WcCategoryForm> = {}): Promise<WcCategoryForm> {
  const el = document.createElement('wc-category-form');
  Object.assign(el, { value: EMPTY_CATEGORY_FORM }, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('validateCategoryForm', () => {
  it('requires a name and nothing else', () => {
    expect(validateCategoryForm({ ...filled, name: ' ' })).toEqual({
      name: 'Name is required',
    });
    expect(validateCategoryForm({ ...filled, taxLine: '', formLine: '' })).toEqual({});
  });
});

describe('formLineWarning', () => {
  it.each(['1120S-1a', '1120S-2', '1120S-5', '1120S-19', 'K-16d', 'excluded', ''])(
    'says nothing about %s',
    (line) => {
      expect(formLineWarning(line)).toBeNull();
    },
  );

  it.each(['1120s-19', 'k-16d', 'Line 19', '1120S'])(
    'warns about %s, which the worksheet matches literally',
    (line) => {
      expect(formLineWarning(line)).toContain('Needs mapping');
    },
  );

  it('is advisory: the value is still a legal one to save', () => {
    // form_line is free text in the CLI, the TUI and the API, and the K-1
    // report has defined behaviour for a value it does not know.
    expect(validateCategoryForm({ ...filled, formLine: '1120s-19' })).toEqual({});
  });
});

describe('formLineSuggestions', () => {
  it('unions the anchors with what the chart of accounts already uses', () => {
    expect(formLineSuggestions(['1120S-19', 'K-16d', null, '1120S-2'])).toEqual([
      '1120S-19',
      '1120S-1a',
      '1120S-2',
      '1120S-5',
      'K-16d',
      'excluded',
    ]);
  });
});

describe('wc-category-form', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('defaults a new category to expense, as the TUI does', async () => {
    const el = await mount();
    expect(el.value.categoryType).toBe('expense');
    expect(
      el.shadowRoot?.querySelector('[data-type]')?.getAttribute('value'),
    ).toBe('expense');
  });

  it('offers both types as radios', async () => {
    const el = await mount();
    const values = [...(el.shadowRoot?.querySelectorAll('wa-radio') ?? [])].map((radio) =>
      radio.getAttribute('value'),
    );
    expect(values).toEqual(['income', 'expense']);
  });

  it('emits the whole value on every edit', async () => {
    const el = await mount({ value: filled });
    const seen: CategoryFormValue[] = [];
    el.addEventListener('nc-category-form-change', (event) =>
      seen.push((event as CustomEvent<NcCategoryFormChangeDetail>).detail.value),
    );

    const input = el.shadowRoot?.querySelector<HTMLInputElement>('[data-tax-line]');
    input!.value = 'Gross receipts';
    input!.dispatchEvent(new Event('input'));

    expect(seen).toEqual([{ ...filled, taxLine: 'Gross receipts' }]);
  });

  it('suggests form lines through a datalist the field can actually reach', async () => {
    const el = await mount({ suggestions: ['1120S-19', 'excluded'] });
    const field = el.shadowRoot?.querySelector('[data-form-line]');
    expect(field?.getAttribute('list')).toBe('form-line-options');
    const options = [...(el.shadowRoot?.querySelectorAll('#form-line-options option') ?? [])].map(
      (option) => option.getAttribute('value'),
    );
    expect(options).toEqual(['1120S-19', 'excluded']);
  });

  it('documents the vocabulary next to the field', async () => {
    const el = await mount();
    const hint = el.shadowRoot?.querySelector('.hint')?.textContent ?? '';
    for (const token of ['1120S-1a', '1120S-2', '1120S-5', 'excluded']) {
      expect(hint).toContain(token);
    }
  });

  it('shows the warning for an unrecognized form line', async () => {
    const el = await mount({ value: { ...filled, formLine: '1120s-19' } });
    expect(el.shadowRoot?.querySelector('.warning')?.textContent).toContain(
      'Needs mapping',
    );
  });

  it('shows no warning for a recognized one', async () => {
    const el = await mount({ value: filled });
    expect(el.shadowRoot?.querySelector('.warning')).toBeNull();
  });
});

describePreviewA11y(preview);

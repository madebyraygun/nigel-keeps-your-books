import { describe, it, expect, afterEach } from 'vitest';
import './wc-category-picker.js';
import type { WcCategoryPicker, NcCategoryChangeDetail } from './wc-category-picker.js';
import { categoryLabel, type CategoryOption } from './category-option.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-category-picker.preview.js';

const categories: CategoryOption[] = [
  { id: 3, name: 'Consulting income', categoryType: 'income' },
  { id: 12, name: 'Software / Subscriptions', categoryType: 'expense' },
  { id: 13, name: 'Meals', categoryType: 'expense' },
];

async function mount(
  props: Partial<WcCategoryPicker> = {},
): Promise<WcCategoryPicker> {
  const el = document.createElement('wc-category-picker');
  el.options = categories;
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function input(el: WcCategoryPicker): HTMLInputElement {
  const found = el.shadowRoot?.querySelector('input');
  if (!found) throw new Error('no combobox input');
  return found;
}

async function type(el: WcCategoryPicker, value: string): Promise<void> {
  const field = input(el);
  field.value = value;
  field.dispatchEvent(new Event('input'));
  await el.updateComplete;
}

async function press(el: WcCategoryPicker, key: string): Promise<void> {
  input(el).dispatchEvent(new KeyboardEvent('keydown', { key, cancelable: true }));
  await el.updateComplete;
}

function optionTexts(el: WcCategoryPicker): string[] {
  return [...(el.shadowRoot?.querySelectorAll('[role="option"]') ?? [])].map((node) =>
    node.textContent?.trim() ?? '',
  );
}

describe('categoryLabel', () => {
  it('tags income and expense the way the TUI does', () => {
    expect(categoryLabel(categories[0])).toBe('Consulting income (inc)');
    expect(categoryLabel(categories[1])).toBe('Software / Subscriptions (exp)');
  });
});

describe('wc-category-picker', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('is a combobox with the list closed until it is opened', async () => {
    const el = await mount();
    expect(input(el).getAttribute('role')).toBe('combobox');
    expect(input(el).getAttribute('aria-expanded')).toBe('false');
    expect(el.shadowRoot?.querySelector('[role="listbox"]')).toBeNull();
  });

  it('groups options income first, then expense', async () => {
    const el = await mount({ listOpen: true });
    const groups = [...(el.shadowRoot?.querySelectorAll('[role="group"]') ?? [])];
    expect(groups.map((g) => g.getAttribute('aria-label'))).toEqual([
      'Income',
      'Expense',
    ]);
    expect(optionTexts(el)).toEqual([
      'Consulting income (inc)',
      'Software / Subscriptions (exp)',
      'Meals (exp)',
    ]);
  });

  it('filters case-insensitively over the label, including the type tag', async () => {
    const el = await mount({ listOpen: true });
    await type(el, 'inc');
    expect(optionTexts(el)).toEqual(['Consulting income (inc)']);

    await type(el, 'meals');
    expect(optionTexts(el)).toEqual(['Meals (exp)']);
  });

  it('says so when nothing matches', async () => {
    const el = await mount({ listOpen: true });
    await type(el, 'zzz');
    expect(optionTexts(el)).toEqual([]);
    expect(el.shadowRoot?.querySelector('.no-matches')?.textContent).toContain(
      'No matching categories',
    );
  });

  it('moves the active option with the arrow keys, in the order drawn', async () => {
    const el = await mount({ listOpen: true });
    await press(el, 'ArrowDown');
    expect(input(el).getAttribute('aria-activedescendant')).toBe('category-option-12');

    await press(el, 'ArrowDown');
    expect(input(el).getAttribute('aria-activedescendant')).toBe('category-option-13');

    await press(el, 'ArrowUp');
    expect(input(el).getAttribute('aria-activedescendant')).toBe('category-option-12');
  });

  it('selects with Enter and emits the choice', async () => {
    const el = await mount({ listOpen: true });
    const seen: NcCategoryChangeDetail[] = [];
    el.addEventListener('nc-category-change', (event) =>
      seen.push((event as CustomEvent<NcCategoryChangeDetail>).detail),
    );

    await press(el, 'ArrowDown');
    await press(el, 'Enter');

    expect(seen).toEqual([{ categoryId: 12, name: 'Software / Subscriptions' }]);
    expect(el.value).toBe(12);
    expect(input(el).value).toBe('Software / Subscriptions');
    expect(input(el).getAttribute('aria-expanded')).toBe('false');
  });

  it('selects on mousedown, which fires before the blur that closes the list', async () => {
    const el = await mount({ listOpen: true });
    const seen: NcCategoryChangeDetail[] = [];
    el.addEventListener('nc-category-change', (event) =>
      seen.push((event as CustomEvent<NcCategoryChangeDetail>).detail),
    );

    el.shadowRoot
      ?.querySelectorAll('[role="option"]')[0]
      ?.dispatchEvent(new MouseEvent('mousedown', { cancelable: true, bubbles: true }));
    await el.updateComplete;

    expect(seen).toEqual([{ categoryId: 3, name: 'Consulting income' }]);
  });

  it('un-selects when typing past a selection, so no stale id can be submitted', async () => {
    const el = await mount({ listOpen: true, value: 12 });
    const seen: NcCategoryChangeDetail[] = [];
    el.addEventListener('nc-category-change', (event) =>
      seen.push((event as CustomEvent<NcCategoryChangeDetail>).detail),
    );

    await type(el, 'Softw');

    expect(el.value).toBeNull();
    expect(seen).toEqual([{ categoryId: null, name: null }]);
  });

  it('leaves Enter and Escape alone when the list is closed', async () => {
    const el = await mount();
    const enter = new KeyboardEvent('keydown', { key: 'Enter', cancelable: true });
    const escape = new KeyboardEvent('keydown', { key: 'Escape', cancelable: true });

    input(el).dispatchEvent(enter);
    input(el).dispatchEvent(escape);
    await el.updateComplete;

    // Not swallowed: the form still submits on Enter and the screen still
    // gets its Escape for Back.
    expect(enter.defaultPrevented).toBe(false);
    expect(escape.defaultPrevented).toBe(false);
  });

  it('closes on Escape while the list is open, without disturbing the selection', async () => {
    const el = await mount({ listOpen: true, value: 12 });
    await press(el, 'Escape');
    expect(el.listOpen).toBe(false);
    expect(el.value).toBe(12);
  });

  it('reads back the selected name when the value is set from outside', async () => {
    const el = await mount();
    el.value = 13;
    await el.updateComplete;
    expect(input(el).value).toBe('Meals');
  });

  it('reset clears the selection and the filter', async () => {
    const el = await mount({ listOpen: true, value: 12 });
    el.reset();
    await el.updateComplete;

    expect(el.value).toBeNull();
    expect(input(el).value).toBe('');
    expect(el.listOpen).toBe(false);
  });

  it('marks itself invalid for the surrounding form', async () => {
    const el = await mount({ invalid: true });
    expect(input(el).getAttribute('aria-invalid')).toBe('true');
  });
});

describePreviewA11y(preview);

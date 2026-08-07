import { describe, it, expect, afterEach } from 'vitest';
import './wc-rule-form.js';
import {
  EMPTY_RULE_FORM,
  MATCH_TYPES,
  matchTypeLabel,
  validateRuleForm,
  type NcRuleFormChangeDetail,
  type RuleFormValue,
  type WcRuleForm,
} from './wc-rule-form.js';
import type { CategoryOption } from './category-option.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-rule-form.preview.js';

const categories: CategoryOption[] = [
  { id: 3, name: 'Consulting income', categoryType: 'income' },
  { id: 12, name: 'Software / Subscriptions', categoryType: 'expense' },
];

const filled: RuleFormValue = {
  pattern: 'ADOBE',
  matchType: 'contains',
  categoryId: 12,
  vendor: 'Adobe',
  priority: 10,
};

async function mount(props: Partial<WcRuleForm> = {}): Promise<WcRuleForm> {
  const el = document.createElement('wc-rule-form');
  Object.assign(el, { value: EMPTY_RULE_FORM, categories }, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function changes(el: WcRuleForm): RuleFormValue[] {
  const seen: RuleFormValue[] = [];
  el.addEventListener('nc-rule-form-change', (event) =>
    seen.push((event as CustomEvent<NcRuleFormChangeDetail>).detail.value),
  );
  return seen;
}

describe('match types', () => {
  it('are the three the categorizer understands, in cli/rules.rs order', () => {
    expect(MATCH_TYPES).toEqual(['contains', 'starts_with', 'regex']);
  });

  it('label a known type and pass an unknown one through', () => {
    expect(matchTypeLabel('starts_with')).toBe('Starts with');
    expect(matchTypeLabel('fuzzy')).toBe('fuzzy');
  });
});

describe('validateRuleForm', () => {
  it('requires a pattern and a category', () => {
    expect(validateRuleForm(EMPTY_RULE_FORM)).toEqual({
      pattern: 'Pattern is required',
      categoryId: 'Choose a category',
    });
    expect(validateRuleForm(filled)).toEqual({});
  });

  it('passes a regex the Rust crate would reject', () => {
    // Deliberate: JS RegExp and the regex crate accept different languages, so
    // a local check would be wrong in both directions. The server decides.
    expect(validateRuleForm({ ...filled, matchType: 'regex', pattern: '(?=x)' })).toEqual(
      {},
    );
  });
});

describe('wc-rule-form', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('offers the three match types with human labels', async () => {
    const el = await mount();
    const options = [...(el.shadowRoot?.querySelectorAll('wa-option') ?? [])].map(
      (option) => [option.getAttribute('value'), option.textContent?.trim()],
    );
    expect(options).toEqual([
      ['contains', 'Contains'],
      ['starts_with', 'Starts with'],
      ['regex', 'Regular expression'],
    ]);
  });

  it('keeps an unrecognized match type as an option rather than silently retyping it', async () => {
    const el = await mount({ value: { ...filled, matchType: 'fuzzy' } });
    const values = [...(el.shadowRoot?.querySelectorAll('wa-option') ?? [])].map(
      (option) => option.getAttribute('value'),
    );
    expect(values).toContain('fuzzy');
  });

  it('says which match types ignore case', async () => {
    const el = await mount();
    const hints = [...(el.shadowRoot?.querySelectorAll('.hint') ?? [])]
      .map((p) => p.textContent)
      .join(' ');
    expect(hints).toContain('ignore case');
    expect(hints).toContain('case-sensitive');
  });

  it('picks the category through the shared picker, not a bare select', async () => {
    const el = await mount({ value: filled });
    const picker = el.shadowRoot?.querySelector('wc-category-picker');
    expect(picker).toBeTruthy();
    expect((picker as unknown as { value: number | null }).value).toBe(12);
  });

  it('emits the category id the picker chose', async () => {
    const el = await mount();
    const seen = changes(el);
    el.shadowRoot?.querySelector('wc-category-picker')?.dispatchEvent(
      new CustomEvent('nc-category-change', {
        detail: { categoryId: 3, name: 'Consulting income' },
        bubbles: true,
        composed: true,
      }),
    );
    expect(seen.at(-1)?.categoryId).toBe(3);
  });

  it('emits the pattern on every keystroke', async () => {
    const el = await mount();
    const seen = changes(el);
    const input = el.shadowRoot?.querySelector<HTMLInputElement>('[data-pattern]');
    input!.value = 'ADO';
    input!.dispatchEvent(new Event('input'));
    input!.value = 'ADOBE';
    input!.dispatchEvent(new Event('input'));
    expect(seen.map((value) => value.pattern)).toEqual(['ADO', 'ADOBE']);
  });

  it('emits priority as a number, and 0 for something unparseable', async () => {
    const el = await mount();
    const seen = changes(el);
    const input = el.shadowRoot?.querySelector<HTMLInputElement>('[data-priority]');

    input!.value = '25';
    input!.dispatchEvent(new Event('input'));
    expect(seen.at(-1)?.priority).toBe(25);

    input!.value = '';
    input!.dispatchEvent(new Event('input'));
    expect(seen.at(-1)?.priority).toBe(0);
  });

  it('renders field errors and marks the picker invalid', async () => {
    const el = await mount({
      errors: { pattern: 'Pattern is required', categoryId: 'Choose a category' },
    });
    const messages = [...(el.shadowRoot?.querySelectorAll('.error') ?? [])].map((p) =>
      p.textContent?.trim(),
    );
    expect(messages).toEqual(['Pattern is required', 'Choose a category']);
    expect(
      el.shadowRoot?.querySelector('wc-category-picker')?.hasAttribute('invalid'),
    ).toBe(true);
  });

  it('exposes a test slot for the live pattern preview', async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelector('slot[name="test"]')).toBeTruthy();
  });
});

describePreviewA11y(preview);

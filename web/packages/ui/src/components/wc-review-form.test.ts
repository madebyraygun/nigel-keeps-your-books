import { describe, it, expect, afterEach } from 'vitest';
import './wc-review-form.js';
import {
  patternPrefill,
  type WcReviewForm,
  type NcReviewApplyDetail,
  type NcRulePatternChangeDetail,
} from './wc-review-form.js';
import type { WcCategoryPicker } from './wc-category-picker.js';
import type { CategoryOption } from './category-option.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-review-form.preview.js';

const categories: CategoryOption[] = [
  { id: 3, name: 'Consulting income', categoryType: 'income' },
  { id: 12, name: 'Software / Subscriptions', categoryType: 'expense' },
  { id: 13, name: 'Meals', categoryType: 'expense' },
];

async function mount(props: Partial<WcReviewForm> = {}): Promise<WcReviewForm> {
  const el = document.createElement('wc-review-form');
  el.categories = categories;
  el.descriptionForPattern = 'ADOBE CREATIVE CLOUD 0000123456';
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function field<T extends Element>(el: WcReviewForm, selector: string): T {
  const found = el.shadowRoot?.querySelector<T>(selector);
  if (!found) throw new Error(`no ${selector}`);
  return found;
}

/** Choose a category the way the picker does, through its own event. */
async function chooseCategory(el: WcReviewForm, id: number): Promise<void> {
  const picker = field<WcCategoryPicker>(el, 'wc-category-picker');
  picker.listOpen = true;
  await picker.updateComplete;
  picker.shadowRoot
    ?.querySelector(`#category-option-${id}`)
    ?.dispatchEvent(new MouseEvent('mousedown', { cancelable: true, bubbles: true }));
  await el.updateComplete;
}

async function typeInto(
  el: WcReviewForm,
  selector: string,
  value: string,
): Promise<void> {
  const input = field<HTMLInputElement>(el, selector);
  input.value = value;
  input.dispatchEvent(new Event('input'));
  await el.updateComplete;
}

async function toggleRule(el: WcReviewForm, on = true): Promise<void> {
  const box = field<HTMLInputElement>(el, '#create-rule');
  box.checked = on;
  box.dispatchEvent(new Event('change'));
  await el.updateComplete;
}

async function submit(el: WcReviewForm): Promise<void> {
  field<HTMLFormElement>(el, 'form').dispatchEvent(
    new Event('submit', { cancelable: true, bubbles: true }),
  );
  await el.updateComplete;
}

function applies(el: WcReviewForm): NcReviewApplyDetail[] {
  const seen: NcReviewApplyDetail[] = [];
  el.addEventListener('nc-review-apply', (event) =>
    seen.push((event as CustomEvent<NcReviewApplyDetail>).detail),
  );
  return seen;
}

describe('patternPrefill', () => {
  it('takes the first two words, as the TUI does', () => {
    expect(patternPrefill('ADOBE CREATIVE CLOUD 0000123456')).toBe('ADOBE CREATIVE');
  });

  it('copes with one word, and with none', () => {
    expect(patternPrefill('RENT')).toBe('RENT');
    expect(patternPrefill('   ')).toBe('');
  });

  it('collapses the runs of spaces a bank statement is full of', () => {
    expect(patternPrefill('SQ   *THE   COFFEE CO')).toBe('SQ *THE');
  });
});

describe('wc-review-form', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('applies with the chosen category and no vendor', async () => {
    const el = await mount();
    const seen = applies(el);

    await chooseCategory(el, 12);
    await submit(el);

    expect(seen).toEqual([
      { categoryId: 12, vendor: null, createRule: false, rulePattern: null },
    ]);
  });

  it('sends a typed vendor, trimmed', async () => {
    const el = await mount();
    const seen = applies(el);

    await chooseCategory(el, 12);
    await typeInto(el, '#vendor', '  Adobe Inc  ');
    await submit(el);

    expect(seen[0].vendor).toBe('Adobe Inc');
  });

  it('refuses to apply without a category, and says why', async () => {
    const el = await mount();
    const seen = applies(el);

    await submit(el);

    expect(seen).toEqual([]);
    expect(field(el, '.field-error').textContent).toContain('Pick a category');
  });

  it('hides the rule half until it is asked for', async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelector('#rule-pattern')).toBeNull();

    await toggleRule(el);
    expect(el.shadowRoot?.querySelector('#rule-pattern')).not.toBeNull();
  });

  it('prefills the pattern with the first two words when the rule opens', async () => {
    const el = await mount();
    await toggleRule(el);
    expect(field<HTMLInputElement>(el, '#rule-pattern').value).toBe('ADOBE CREATIVE');
  });

  it('announces the pattern so the screen can test it, and retracts it on close', async () => {
    const el = await mount();
    const seen: NcRulePatternChangeDetail[] = [];
    el.addEventListener('nc-rule-pattern-change', (event) =>
      seen.push((event as CustomEvent<NcRulePatternChangeDetail>).detail),
    );

    await toggleRule(el);
    await typeInto(el, '#rule-pattern', 'ADOBE');
    await toggleRule(el, false);

    expect(seen.map((d) => d.pattern)).toEqual(['ADOBE CREATIVE', 'ADOBE', '']);
  });

  it('applies with the rule when one is asked for', async () => {
    const el = await mount();
    const seen = applies(el);

    await chooseCategory(el, 12);
    await toggleRule(el);
    await typeInto(el, '#rule-pattern', 'ADOBE');
    await submit(el);

    expect(seen).toEqual([
      { categoryId: 12, vendor: null, createRule: true, rulePattern: 'ADOBE' },
    ]);
  });

  it('blocks a blank pattern rather than letting the server 400 it', async () => {
    const el = await mount();
    const seen = applies(el);

    await chooseCategory(el, 12);
    await toggleRule(el);
    await typeInto(el, '#rule-pattern', '   ');
    await submit(el);

    expect(seen).toEqual([]);
    expect(field(el, '.field-error').textContent).toContain('needs a pattern');
  });

  it('does not apply twice while one is in flight', async () => {
    const el = await mount();
    const seen = applies(el);

    await chooseCategory(el, 12);
    el.busy = true;
    await el.updateComplete;
    await submit(el);

    expect(seen).toEqual([]);
  });

  it('emits skip and back', async () => {
    const el = await mount({ canGoBack: true });
    const seen: string[] = [];
    el.addEventListener('nc-review-skip', () => seen.push('skip'));
    el.addEventListener('nc-review-back', () => seen.push('back'));

    const buttons = [...(el.shadowRoot?.querySelectorAll('button') ?? [])];
    buttons.find((b) => b.textContent?.includes('Skip'))?.click();
    buttons.find((b) => b.textContent?.includes('Back'))?.click();

    expect(seen).toEqual(['skip', 'back']);
  });

  it('disables Back on the first transaction', async () => {
    const el = await mount({ canGoBack: false });
    const back = [...(el.shadowRoot?.querySelectorAll('button') ?? [])].find((b) =>
      b.textContent?.includes('Back'),
    );
    expect(back?.disabled).toBe(true);
  });

  it('shows a server rejection as an alert', async () => {
    const el = await mount({ error: '`rulePattern` is required.' });
    const alert = field(el, '[role="alert"]');
    expect(alert.textContent).toContain('rulePattern');
  });

  it('reset clears every field for the next transaction', async () => {
    const el = await mount();
    await chooseCategory(el, 12);
    await typeInto(el, '#vendor', 'Adobe Inc');
    await toggleRule(el);

    el.reset();
    await el.updateComplete;

    expect(field<HTMLInputElement>(el, '#vendor').value).toBe('');
    expect(field<HTMLInputElement>(el, '#create-rule').checked).toBe(false);
    expect(el.shadowRoot?.querySelector('#rule-pattern')).toBeNull();

    const seen = applies(el);
    await submit(el);
    expect(seen).toEqual([]);
  });

  it('offers the rule preview a slot to arrive in', async () => {
    const el = await mount();
    await toggleRule(el);
    expect(el.shadowRoot?.querySelector('slot[name="rule-test"]')).not.toBeNull();
  });

  it('documents the two keys it actually binds', async () => {
    const el = await mount();
    const keys = field(el, '.keys').textContent?.replace(/\s+/g, ' ');
    expect(keys).toContain('Enter');
    expect(keys).toContain('Esc');
    // Tab is the browser's focus key here, not a skip key.
    expect(keys).not.toContain('Tab');
  });
});

describePreviewA11y(preview);

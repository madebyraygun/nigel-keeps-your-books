import { describe, it, expect, afterEach, vi } from 'vitest';
import './rules.js';
import type { NigelRulesScreen } from './rules.js';
import type {
  WcManagerDialog,
  WcManagerLayout,
  WcManagerTable,
  WcRuleForm,
  WcRuleTestPreview,
} from '@nigel/ui';
import { ApiError } from '../api/index.js';
import { conflictError, FakeApiClient } from '../__mocks__/fake-api-client.js';
import type { CategoryRow, RuleRow, RuleTestResult } from '../api/types.js';
import type { ScreenId } from './registry.js';

const CATEGORIES: CategoryRow[] = [
  {
    id: 3,
    name: 'Consulting income',
    categoryType: 'income',
    taxLine: null,
    formLine: null,
  },
  {
    id: 12,
    name: 'Software / Subscriptions',
    categoryType: 'expense',
    taxLine: null,
    formLine: null,
  },
];

const RULES: RuleRow[] = [
  {
    id: 7,
    pattern: 'ADOBE',
    matchType: 'contains',
    vendor: 'Adobe',
    category: 'Software / Subscriptions',
    categoryId: 12,
    priority: 10,
    hitCount: 42,
  },
  {
    id: 8,
    pattern: 'SQ *',
    matchType: 'starts_with',
    vendor: null,
    category: 'Consulting income',
    categoryId: 3,
    priority: 0,
    hitCount: 3,
  },
];

function client(rules: RuleRow[] = RULES): FakeApiClient {
  const fake = new FakeApiClient();
  fake.rules = rules.map((rule) => ({ ...rule }));
  fake.categories = CATEGORIES.map((category) => ({ ...category }));
  return fake;
}

async function settle(el: NigelRulesScreen): Promise<void> {
  await el.updateComplete;
  await new Promise((resolve) => setTimeout(resolve, 0));
  await el.updateComplete;
}

interface Mounted {
  el: NigelRulesScreen;
  fake: FakeApiClient;
  navigations: { screen: ScreenId; params?: URLSearchParams }[];
}

async function mount(fake: FakeApiClient = client(), query = ''): Promise<Mounted> {
  const navigations: Mounted['navigations'] = [];
  const el = document.createElement('nigel-rules-screen');
  el.client = fake;
  el.params = new URLSearchParams(query);
  el.navigate = (screen, params) => navigations.push({ screen, params });
  document.body.appendChild(el);
  await settle(el);
  return { el, fake, navigations };
}

function layout(el: NigelRulesScreen): WcManagerLayout {
  const found = el.shadowRoot?.querySelector<WcManagerLayout>('wc-manager-layout');
  if (!found) throw new Error('no layout on screen');
  return found;
}

function table(el: NigelRulesScreen): WcManagerTable {
  const found = el.shadowRoot?.querySelector<WcManagerTable>('wc-manager-table');
  if (!found) throw new Error('no table on screen');
  return found;
}

function dialog(el: NigelRulesScreen): WcManagerDialog | null {
  return el.shadowRoot?.querySelector<WcManagerDialog>('wc-manager-dialog') ?? null;
}

function form(el: NigelRulesScreen): WcRuleForm {
  const found = dialog(el)?.querySelector<WcRuleForm>('wc-rule-form');
  if (!found) throw new Error('no rule form on screen');
  return found;
}

function testPanel(el: NigelRulesScreen): WcRuleTestPreview {
  const found = dialog(el)?.querySelector<WcRuleTestPreview>('wc-rule-test-preview');
  if (!found) throw new Error('no rule test panel on screen');
  return found;
}

/** Change the form the way the component would, without the DOM in between. */
async function change(
  el: NigelRulesScreen,
  patch: Partial<WcRuleForm['value']>,
): Promise<void> {
  const target = form(el);
  target.dispatchEvent(
    new CustomEvent('nc-rule-form-change', {
      detail: { value: { ...target.value, ...patch } },
      bubbles: true,
      composed: true,
    }),
  );
  await settle(el);
}

async function openAdd(el: NigelRulesScreen): Promise<void> {
  layout(el).dispatchEvent(new CustomEvent('nc-manager-add'));
  await settle(el);
}

async function rowAction(el: NigelRulesScreen, action: string, id: number): Promise<void> {
  table(el).dispatchEvent(
    new CustomEvent('nc-manager-action', {
      detail: { action, id },
      bubbles: true,
      composed: true,
    }),
  );
  await settle(el);
}

async function save(el: NigelRulesScreen): Promise<void> {
  dialog(el)?.dispatchEvent(new CustomEvent('nc-manager-save'));
  await settle(el);
}

async function confirmDeletion(answer: boolean): Promise<void> {
  const ui = await import('@nigel/ui');
  vi.spyOn(ui, 'confirmDialog').mockResolvedValue(answer);
}

describe('nigel-rules-screen', () => {
  afterEach(() => {
    document.body.innerHTML = '';
    vi.restoreAllMocks();
  });

  it('lists rules in the order the server sends them — the order they apply', async () => {
    const { el } = await mount();
    expect(table(el).rows.map((row) => row.cells)).toEqual([
      ['ADOBE', 'Contains', 'Software / Subscriptions', 'Adobe', 10, 42],
      ['SQ *', 'Starts with', 'Consulting income', null, 0, 3],
    ]);
  });

  it('shows the empty state with a pointer at review', async () => {
    const { el } = await mount(client([]));
    expect(layout(el).empty).toBe(true);
    const empty = el.shadowRoot?.querySelector('wc-empty-state');
    expect(empty?.getAttribute('heading')).toBe('No rules yet');
    expect(empty?.querySelector('a')?.getAttribute('href')).toBe('#/review');
  });

  it('creates a rule, sending match type and priority explicitly', async () => {
    const { el, fake } = await mount();
    await openAdd(el);
    await change(el, { pattern: 'RENT', categoryId: 12, priority: 5 });
    await save(el);

    expect(fake.calls).toContain(
      'createRule:{"pattern":"RENT","categoryId":12,"vendor":null,"matchType":"contains","priority":5}',
    );
    // Refetched rather than spliced: a priority edit reorders the list.
    expect(fake.calls.filter((call) => call === 'getRules')).toHaveLength(2);
  });

  it('will not send a rule with no pattern or no category', async () => {
    const { el, fake } = await mount();
    await openAdd(el);
    await save(el);

    expect(fake.calls.some((call) => call.startsWith('createRule'))).toBe(false);
    expect(form(el).errors).toEqual({
      pattern: 'Pattern is required',
      categoryId: 'Choose a category',
    });
  });

  it('edits with a partial patch', async () => {
    const { el, fake } = await mount();
    await rowAction(el, 'edit', 7);
    await change(el, { priority: 25 });
    await save(el);

    expect(fake.calls[2]).toBe('updateRule:7:{"priority":25}');
  });

  it('clears a vendor with an explicit null', async () => {
    const { el, fake } = await mount();
    await rowAction(el, 'edit', 7);
    await change(el, { vendor: '' });
    await save(el);

    expect(fake.calls[2]).toBe('updateRule:7:{"vendor":null}');
  });

  it('issues no request when an edit changes nothing', async () => {
    const { el, fake } = await mount();
    await rowAction(el, 'edit', 7);
    await save(el);

    expect(fake.calls.some((call) => call.startsWith('updateRule'))).toBe(false);
    expect(dialog(el)).toBeNull();
  });

  it('deletes once confirmed, and refetches', async () => {
    await confirmDeletion(true);
    const { el, fake } = await mount();
    await rowAction(el, 'delete', 8);

    expect(fake.calls).toContain('deleteRule:8');
    expect(table(el).rows).toHaveLength(1);
  });

  it('treats an already-deleted rule as a stale list, and refreshes it', async () => {
    await confirmDeletion(true);
    const fake = client();
    fake.deleteRuleError = conflictError('already_inactive');
    const { el } = await mount(fake);
    await rowAction(el, 'delete', 7);

    expect(layout(el).error).toBe(
      'This rule has already been deleted. The list has been refreshed.',
    );
    expect(fake.calls.filter((call) => call === 'getRules')).toHaveLength(2);
  });

  describe('the live pattern test', () => {
    it('asks once for a burst of typing, with the last pattern', async () => {
      vi.useFakeTimers();
      try {
        const fake = client();
        fake.ruleTest = { total: 3, matches: [{ description: 'ADOBE CC', count: 3 }] };
        const el = document.createElement('nigel-rules-screen');
        el.client = fake;
        document.body.appendChild(el);
        await vi.advanceTimersByTimeAsync(0);
        await el.updateComplete;

        layout(el).dispatchEvent(new CustomEvent('nc-manager-add'));
        await el.updateComplete;

        for (const pattern of ['A', 'AD', 'ADOBE']) {
          form(el).dispatchEvent(
            new CustomEvent('nc-rule-form-change', {
              detail: { value: { ...form(el).value, pattern } },
              bubbles: true,
              composed: true,
            }),
          );
          await el.updateComplete;
        }

        expect(fake.calls.filter((call) => call.startsWith('testRule'))).toHaveLength(0);

        await vi.advanceTimersByTimeAsync(250);
        await el.updateComplete;

        expect(fake.calls.filter((call) => call.startsWith('testRule'))).toEqual([
          'testRule:{"pattern":"ADOBE","matchType":"contains"}',
        ]);
      } finally {
        vi.useRealTimers();
      }
    });

    it('asks immediately when the match type changes', async () => {
      // A click is a decision, not typing; waiting on it reads as a bug.
      const { el, fake } = await mount();
      await openAdd(el);
      await change(el, { pattern: 'ADOBE' });
      fake.calls.length = 0;
      await change(el, { matchType: 'regex' });

      expect(fake.calls).toEqual(['testRule:{"pattern":"ADOBE","matchType":"regex"}']);
    });

    it('asks nothing about a blank or whitespace pattern', async () => {
      // The route's guard is is_empty(), not a trim, so "  " would be scanned
      // against every description in the database for nothing.
      const { el, fake } = await mount();
      await openAdd(el);
      await change(el, { pattern: '   ' });
      await new Promise((resolve) => setTimeout(resolve, 300));

      expect(fake.calls.some((call) => call.startsWith('testRule'))).toBe(false);
      expect(testPanel(el).result).toBeNull();
    });

    it('renders what the pattern matches', async () => {
      const fake = client();
      fake.ruleTest = { total: 3, matches: [{ description: 'ADOBE CC', count: 3 }] };
      const { el } = await mount(fake);
      await openAdd(el);
      await change(el, { pattern: 'ADOBE' });
      await new Promise((resolve) => setTimeout(resolve, 300));
      await settle(el);

      expect(testPanel(el).result?.total).toBe(3);
    });

    it('renders an invalid regex inline and still lets the rule be saved', async () => {
      const fake = client();
      fake.ruleTestError = new ApiError({
        code: 'bad_request',
        rawCode: 'bad_request',
        message: 'Invalid regex: unclosed group',
        status: 400,
      });
      const { el } = await mount(fake);
      await openAdd(el);
      await change(el, { pattern: '(', matchType: 'regex', categoryId: 12 });
      await new Promise((resolve) => setTimeout(resolve, 300));
      await settle(el);

      expect(testPanel(el).error).toBe('Invalid regex: unclosed group');

      // The preview failing is not the form failing: the server decides both,
      // and a good pattern typed next still saves.
      await change(el, { pattern: 'ADOBE', matchType: 'contains' });
      await save(el);
      expect(fake.calls.some((call) => call.startsWith('createRule'))).toBe(true);
    });

    it('drops an answer whose pattern is no longer the current one', async () => {
      const fake = client();
      // Held on an object rather than in a local: a `let` assigned only inside
      // the executor is still narrowed to its initializer at the call site.
      const first: { resolve?: (value: RuleTestResult) => void } = {};
      const originalTestRule = fake.testRule.bind(fake);
      let call = 0;
      fake.testRule = (input) => {
        call += 1;
        if (call === 1) {
          return new Promise<RuleTestResult>((resolve) => {
            first.resolve = resolve;
          });
        }
        return originalTestRule(input);
      };

      const { el } = await mount(fake);
      await openAdd(el);
      await change(el, { pattern: 'ADOBE' });
      await new Promise((resolve) => setTimeout(resolve, 300));

      await change(el, { pattern: 'RENT' });
      await new Promise((resolve) => setTimeout(resolve, 300));
      await settle(el);

      first.resolve?.({ total: 99, matches: [] });
      await settle(el);

      expect(testPanel(el).result?.total).not.toBe(99);
    });
  });

  describe('the category filter', () => {
    it('shows only that category’s rules, and says so', async () => {
      const { el } = await mount(client(), 'categoryId=12');
      expect(table(el).rows.map((row) => row.id)).toEqual([7]);
      expect(
        el.shadowRoot?.querySelector('[data-filter]')?.textContent,
      ).toContain('Showing rules for Software / Subscriptions');
    });

    it('clears itself back to the whole list', async () => {
      const { el, navigations } = await mount(client(), 'categoryId=12');
      el.shadowRoot?.querySelector<HTMLButtonElement>('[data-filter] button')?.click();
      await settle(el);

      expect(navigations).toEqual([{ screen: 'rules', params: undefined }]);
    });

    it('does not refetch when the filter changes', async () => {
      // The rules endpoint has no category filter, and a screen may not invent
      // one; this is a pass over the list already in hand.
      const { el, fake } = await mount();
      fake.calls.length = 0;
      el.params = new URLSearchParams('categoryId=12');
      await settle(el);

      expect(fake.calls).toEqual([]);
      expect(table(el).rows.map((row) => row.id)).toEqual([7]);
    });

    it('says something useful when the filter matches nothing', async () => {
      const { el } = await mount(client(), 'categoryId=999');
      expect(layout(el).empty).toBe(true);
      expect(el.shadowRoot?.querySelector('wc-empty-state')?.getAttribute('heading')).toBe(
        'No rules for that category',
      );
    });
  });
});

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import './review.js';
import type { NigelReviewScreen } from './review.js';
import type {
  WcReviewCard,
  WcReviewForm,
  WcReviewProgress,
  WcRuleTestPreview,
  NcReviewApplyDetail,
} from '@nigel/ui';
import { ApiError } from '../api/index.js';
import { FakeApiClient } from '../__mocks__/fake-api-client.js';
import type { CategoryRow, FlaggedTxn, RegisterRow } from '../api/types.js';

const categories: CategoryRow[] = [
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

function flagged(id: number, description: string): FlaggedTxn {
  return {
    id,
    date: '2025-03-0' + id,
    description,
    amount: -10 * id,
    accountName: 'BofA Credit Card',
  };
}

function rowFor(txn: FlaggedTxn): RegisterRow {
  return {
    ...txn,
    category: null,
    categoryId: null,
    vendor: null,
    isFlagged: true,
  };
}

const QUEUE = [
  flagged(1, 'ADOBE CREATIVE CLOUD 0000123'),
  flagged(2, 'SQ *COFFEE BAR'),
  flagged(3, 'RENT MARCH 2025'),
];

function client(queue: FlaggedTxn[] = QUEUE): FakeApiClient {
  const fake = new FakeApiClient();
  fake.reviewQueue = queue;
  fake.categories = categories;
  for (const txn of queue) fake.reviewRows.set(txn.id, rowFor(txn));
  return fake;
}

async function settle(el: NigelReviewScreen): Promise<void> {
  await el.updateComplete;
  await new Promise((resolve) => setTimeout(resolve, 0));
  await el.updateComplete;
}

async function mount(
  fake: FakeApiClient = client(),
  query = '',
): Promise<{ el: NigelReviewScreen; fake: FakeApiClient }> {
  const el = document.createElement('nigel-review-screen');
  el.client = fake;
  el.params = new URLSearchParams(query);
  document.body.appendChild(el);
  await settle(el);
  return { el, fake };
}

function card(el: NigelReviewScreen): WcReviewCard | null {
  return el.shadowRoot?.querySelector('wc-review-card') ?? null;
}

function form(el: NigelReviewScreen): WcReviewForm {
  const found = el.shadowRoot?.querySelector<WcReviewForm>('wc-review-form');
  if (!found) throw new Error('no review form on screen');
  return found;
}

function progressText(el: NigelReviewScreen): string {
  const bar = el.shadowRoot?.querySelector<WcReviewProgress>('wc-review-progress');
  return bar?.shadowRoot?.querySelector('.position')?.textContent?.trim() ?? '';
}

/** Drive the screen the way the form does, through the form's own events. */
async function apply(
  el: NigelReviewScreen,
  detail: Partial<NcReviewApplyDetail> = {},
): Promise<void> {
  form(el).dispatchEvent(
    new CustomEvent<NcReviewApplyDetail>('nc-review-apply', {
      detail: {
        categoryId: 12,
        vendor: null,
        createRule: false,
        rulePattern: null,
        ...detail,
      },
      bubbles: true,
      composed: true,
    }),
  );
  await settle(el);
}

async function skip(el: NigelReviewScreen): Promise<void> {
  form(el).dispatchEvent(new CustomEvent('nc-review-skip', { bubbles: true, composed: true }));
  await settle(el);
}

async function back(el: NigelReviewScreen): Promise<void> {
  form(el).dispatchEvent(new CustomEvent('nc-review-back', { bubbles: true, composed: true }));
  await settle(el);
}

async function backFromSummary(el: NigelReviewScreen): Promise<void> {
  const button = el.shadowRoot?.querySelector<HTMLButtonElement>('button.undo-last');
  if (!button) throw new Error('no back button on the summary');
  button.click();
  await settle(el);
}

async function patternChange(el: NigelReviewScreen, pattern: string): Promise<void> {
  form(el).dispatchEvent(
    new CustomEvent('nc-rule-pattern-change', {
      detail: { pattern },
      bubbles: true,
      composed: true,
    }),
  );
  await el.updateComplete;
}

function summaryCounts(el: NigelReviewScreen): Record<string, string> {
  const terms = [...(el.shadowRoot?.querySelectorAll('.summary-counts dt') ?? [])];
  const values = [...(el.shadowRoot?.querySelectorAll('.summary-counts dd') ?? [])];
  return Object.fromEntries(
    terms.map((term, i) => [
      term.textContent?.trim() ?? '',
      values[i]?.textContent?.trim() ?? '',
    ]),
  );
}

const toasts: string[] = [];
const onToast = (event: Event) => {
  toasts.push((event as CustomEvent<{ message: string }>).detail.message);
};

describe('nigel-review-screen', () => {
  beforeEach(() => {
    toasts.length = 0;
    window.addEventListener('nc-toast', onToast);
  });

  afterEach(() => {
    window.removeEventListener('nc-toast', onToast);
    document.body.innerHTML = '';
  });

  it('opens on the first flagged transaction', async () => {
    const { el, fake } = await mount();
    expect(fake.calls).toContain('getReviewQueue');
    expect(card(el)?.description).toBe('ADOBE CREATIVE CLOUD 0000123');
    expect(progressText(el)).toBe('1 of 3');
  });

  it('applies the decision and moves on', async () => {
    const { el, fake } = await mount();
    await apply(el, { categoryId: 12, vendor: 'Adobe Inc' });

    expect(fake.calls).toContain(
      'applyReview:1:{"categoryId":12,"vendor":"Adobe Inc"}',
    );
    expect(card(el)?.description).toBe('SQ *COFFEE BAR');
    expect(progressText(el)).toBe('2 of 3');
    expect(fake.reviewRows.get(1)?.isFlagged).toBe(false);
  });

  it('omits the vendor entirely when none was typed', async () => {
    const { el, fake } = await mount();
    await apply(el, { vendor: null });
    expect(fake.calls).toContain('applyReview:1:{"categoryId":12}');
  });

  it('sends the rule alongside the decision', async () => {
    const { el, fake } = await mount();
    await apply(el, { createRule: true, rulePattern: 'ADOBE CREATIVE' });

    expect(fake.calls).toContain(
      'applyReview:1:{"categoryId":12,"createRule":true,"rulePattern":"ADOBE CREATIVE"}',
    );
  });

  it('undoes the right rule when stepping back, and re-presents the transaction', async () => {
    const { el, fake } = await mount();
    await apply(el, { createRule: true, rulePattern: 'ADOBE CREATIVE' });
    expect(card(el)?.description).toBe('SQ *COFFEE BAR');

    await back(el);

    // The ruleId the apply answered with, not a guess.
    expect(fake.calls).toContain('undoReview:1:{"ruleId":100}');
    expect(card(el)?.description).toBe('ADOBE CREATIVE CLOUD 0000123');
    expect(progressText(el)).toBe('1 of 3');
    // And the server-side row really is back in the queue's state.
    expect(fake.reviewRows.get(1)?.isFlagged).toBe(true);
    expect(fake.reviewRows.get(1)?.categoryId).toBeNull();
  });

  it('undoes without a ruleId when the decision made no rule', async () => {
    const { el, fake } = await mount();
    await apply(el);
    await back(el);
    expect(fake.calls).toContain('undoReview:1:{}');
  });

  it('skips without touching the server at all', async () => {
    const { el, fake } = await mount();
    await skip(el);

    expect(fake.calls.some((call) => call.startsWith('applyReview'))).toBe(false);
    expect(fake.calls.some((call) => call.startsWith('undoReview'))).toBe(false);
    expect(fake.reviewRows.get(1)?.isFlagged).toBe(true);
    expect(card(el)?.description).toBe('SQ *COFFEE BAR');
  });

  it('steps back over a skip without undoing anything', async () => {
    const { el, fake } = await mount();
    await skip(el);
    await back(el);

    expect(fake.calls.some((call) => call.startsWith('undoReview'))).toBe(false);
    expect(card(el)?.description).toBe('ADOBE CREATIVE CLOUD 0000123');
  });

  it('cannot step back off the front of the queue', async () => {
    const { el, fake } = await mount();
    await back(el);
    expect(fake.calls.some((call) => call.startsWith('undoReview'))).toBe(false);
    expect(progressText(el)).toBe('1 of 3');
  });

  it('goes back on Escape', async () => {
    const { el, fake } = await mount();
    await apply(el);

    el.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }),
    );
    await settle(el);

    expect(fake.calls).toContain('undoReview:1:{}');
    expect(card(el)?.description).toBe('ADOBE CREATIVE CLOUD 0000123');
  });

  it('counts what happened in the summary', async () => {
    const { el } = await mount();
    await apply(el, { createRule: true, rulePattern: 'ADOBE' });
    await skip(el);
    await apply(el);

    expect(card(el)).toBeNull();
    expect(summaryCounts(el)).toEqual({
      Reviewed: '2',
      Skipped: '1',
      'Rules created': '1',
    });
  });

  it('can take back the last decision from the summary', async () => {
    const { el, fake } = await mount();
    await apply(el, { createRule: true, rulePattern: 'ADOBE' });
    await skip(el);
    await apply(el, { categoryId: 3 });
    expect(summaryCounts(el)['Reviewed']).toBe('2');

    await backFromSummary(el);

    // Back on the last transaction, with its decision undone on the server.
    expect(card(el)?.description).toBe('RENT MARCH 2025');
    expect(fake.calls).toContain('undoReview:3:{}');
    expect(fake.reviewRows.get(3)?.isFlagged).toBe(true);

    await apply(el, { categoryId: 12, createRule: true, rulePattern: 'RENT' });
    expect(summaryCounts(el)).toEqual({
      Reviewed: '2',
      Skipped: '1',
      'Rules created': '2',
    });
  });

  it('links on from the summary', async () => {
    const { el } = await mount(client([QUEUE[0]]));
    await apply(el);

    const hrefs = [...(el.shadowRoot?.querySelectorAll('a') ?? [])].map((a) =>
      a.getAttribute('href'),
    );
    expect(hrefs).toEqual(['#/register', '#/dashboard']);
  });

  describe('a single transaction by id', () => {
    it('fetches only that one', async () => {
      const fake = client();
      const { el } = await mount(fake, 'id=2');

      expect(fake.calls).toContain('getReviewTransaction:2');
      expect(fake.calls).not.toContain('getReviewQueue');
      expect(card(el)?.description).toBe('SQ *COFFEE BAR');
      expect(progressText(el)).toBe('1 of 1');
    });

    it('finishes after one decision', async () => {
      const { el } = await mount(client(), 'id=2');
      await apply(el);
      expect(summaryCounts(el)).toEqual({
        Reviewed: '1',
        Skipped: '0',
        'Rules created': '0',
      });
    });

    it('shows what a re-reviewed transaction already carries', async () => {
      const fake = client();
      fake.reviewRows.set(2, {
        ...rowFor(QUEUE[1]),
        category: 'Meals',
        categoryId: 13,
        vendor: 'Coffee Bar',
        isFlagged: false,
      });
      const { el } = await mount(fake, 'id=2');

      expect(card(el)?.currentCategory).toBe('Meals');
      expect(card(el)?.currentVendor).toBe('Coffee Bar');
    });

    it('says so when the transaction is gone', async () => {
      const fake = client();
      fake.reviewTransactionError = new ApiError({
        code: 'not_found',
        rawCode: 'not_found',
        message: 'No transaction found with ID 99',
        status: 404,
      });
      const { el } = await mount(fake, 'id=99');

      expect(card(el)).toBeNull();
      expect(el.shadowRoot?.querySelector('wc-empty-state')?.getAttribute('message')).toBe(
        'Transaction 99 is not there any more.',
      );
    });

    it('ignores an id that is not a transaction id and reviews the queue', async () => {
      const fake = client();
      await mount(fake, 'id=nonsense');
      expect(fake.calls).toContain('getReviewQueue');
    });
  });

  describe('when things go wrong', () => {
    it('moves past a transaction that has gone, with a toast', async () => {
      const fake = client();
      fake.applyError = new ApiError({
        code: 'not_found',
        rawCode: 'not_found',
        message: 'No transaction found with ID 1',
        status: 404,
      });
      const { el } = await mount(fake);

      await apply(el);

      expect(toasts).toEqual(['That transaction is gone — moving on.']);
      expect(card(el)?.description).toBe('SQ *COFFEE BAR');

      // It counts as a skip, so stepping back does not undo a decision that
      // was never recorded.
      fake.applyError = null;
      await back(el);
      expect(fake.calls.some((call) => call.startsWith('undoReview'))).toBe(false);
    });

    it('shows a rejected decision inline and stays put', async () => {
      const fake = client();
      fake.applyError = new ApiError({
        code: 'bad_request',
        rawCode: 'bad_request',
        message: 'rulePattern is required when createRule is true.',
        status: 400,
      });
      const { el } = await mount(fake);

      await apply(el, { createRule: true, rulePattern: '' });

      expect(form(el).error).toBe('rulePattern is required when createRule is true.');
      expect(card(el)?.description).toBe('ADOBE CREATIVE CLOUD 0000123');
      expect(progressText(el)).toBe('1 of 3');
    });

    it('keeps the stack honest when an undo fails', async () => {
      const fake = client();
      const { el } = await mount(fake);
      await apply(el, { createRule: true, rulePattern: 'ADOBE' });

      fake.undoError = new ApiError({
        code: 'internal',
        rawCode: 'internal',
        message: 'database is locked',
        status: 500,
      });
      await back(el);

      // Still on the second transaction: the decision the server holds is
      // still on the stack, so a later Back will find it.
      expect(card(el)?.description).toBe('SQ *COFFEE BAR');
      expect(progressText(el)).toBe('2 of 3');
      expect(toasts).toContain('database is locked');

      fake.undoError = null;
      await back(el);
      expect(fake.calls).toContain('undoReview:1:{"ruleId":100}');
      expect(card(el)?.description).toBe('ADOBE CREATIVE CLOUD 0000123');
    });

    it('says so when the queue cannot be loaded', async () => {
      const fake = client();
      fake.queueError = new ApiError({
        code: 'internal',
        rawCode: 'internal',
        message: 'database is locked',
        status: 500,
      });
      const { el } = await mount(fake);

      expect(card(el)).toBeNull();
      expect(toasts).toContain('database is locked');
    });

    it('has an empty state for a queue with nothing in it', async () => {
      const { el } = await mount(client([]));
      expect(card(el)).toBeNull();
      expect(el.shadowRoot?.querySelector('wc-empty-state')?.getAttribute('heading')).toBe(
        'Nothing to review',
      );
    });
  });

  describe('the rule test preview', () => {
    beforeEach(() => {
      vi.useFakeTimers();
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    function panel(el: NigelReviewScreen): WcRuleTestPreview {
      const found = el.shadowRoot?.querySelector<WcRuleTestPreview>('wc-rule-test-preview');
      if (!found) throw new Error('no rule test panel');
      return found;
    }

    it('asks once for the last pattern typed', async () => {
      vi.useRealTimers();
      const fake = client();
      fake.ruleTest = { total: 3, matches: [{ description: 'ADOBE CC', count: 3 }] };
      const { el } = await mount(fake);
      vi.useFakeTimers();

      await patternChange(el, 'A');
      await patternChange(el, 'AD');
      await patternChange(el, 'ADOBE');

      await vi.advanceTimersByTimeAsync(300);
      await el.updateComplete;

      const tests = fake.calls.filter((call) => call.startsWith('testRule'));
      expect(tests).toEqual(['testRule:{"pattern":"ADOBE","matchType":"contains"}']);
      expect(panel(el).result).toEqual({
        total: 3,
        matches: [{ description: 'ADOBE CC', count: 3 }],
      });
    });

    it('asks nothing at all for a blank pattern', async () => {
      vi.useRealTimers();
      const fake = client();
      const { el } = await mount(fake);
      vi.useFakeTimers();

      await patternChange(el, 'ADOBE');
      await patternChange(el, '   ');
      await vi.advanceTimersByTimeAsync(300);

      expect(fake.calls.some((call) => call.startsWith('testRule'))).toBe(false);
      expect(panel(el).result).toBeNull();
    });

    it('renders a rejected pattern inline and still lets the decision through', async () => {
      vi.useRealTimers();
      const fake = client();
      fake.ruleTestError = new ApiError({
        code: 'bad_request',
        rawCode: 'bad_request',
        message: 'Invalid regex: unclosed group',
        status: 400,
      });
      const { el } = await mount(fake);
      vi.useFakeTimers();

      await patternChange(el, 'ADOBE(');
      await vi.advanceTimersByTimeAsync(300);
      await el.updateComplete;

      expect(panel(el).error).toBe('Invalid regex: unclosed group');

      vi.useRealTimers();
      await apply(el, { createRule: true, rulePattern: 'ADOBE(' });
      expect(fake.calls.some((call) => call.startsWith('applyReview'))).toBe(true);
    });

    it('forgets the preview when the queue moves on', async () => {
      vi.useRealTimers();
      const fake = client();
      fake.ruleTest = { total: 1, matches: [{ description: 'ADOBE CC', count: 1 }] };
      const { el } = await mount(fake);
      vi.useFakeTimers();

      await patternChange(el, 'ADOBE');
      await vi.advanceTimersByTimeAsync(300);
      await el.updateComplete;
      expect(panel(el).result).not.toBeNull();

      vi.useRealTimers();
      await skip(el);
      expect(panel(el).result).toBeNull();
    });
  });
});

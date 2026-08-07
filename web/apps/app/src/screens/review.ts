import {
  LitElement,
  html,
  css,
  nothing,
  type PropertyValues,
  type TemplateResult,
} from 'lit';
import { customElement, property, state, query } from 'lit/decorators.js';
import '@nigel/ui';
import {
  dispatchNcToast,
  type CategoryOption,
  type NcReviewApplyDetail,
  type NcRulePatternChangeDetail,
  type WcReviewForm,
} from '@nigel/ui';

import { ApiError, type ApiClient } from '../api/index.js';
import type { CategoryRow, RuleTestResult } from '../api/types.js';
import {
  isMissingTransaction,
  singleIdFrom,
  summarize,
  toReviewItem,
  type Decision,
  type ReviewItem,
} from './review-data.js';
import type { ScreenContext } from './context.js';
import type { ScreenId } from './registry.js';

/** How long to sit on a pattern before asking the server what it matches. */
const RULE_TEST_DEBOUNCE_MS = 250;

type Phase = 'loading' | 'empty' | 'reviewing' | 'summary' | 'error';

/**
 * The interactive reviewer — the web counterpart of `reviewer.rs`.
 *
 * The shape of it is the TUI's: one transaction at a time, a decision stack
 * behind you, and a Back that undoes rather than merely re-shows. Undo is a
 * server call (`POST /api/review/:id/undo`) that re-flags the transaction and
 * deletes any rule the decision created, so stepping backwards leaves the
 * database as though the decision had never been made.
 *
 * Two departures from the TUI, both forced by the browser. Tab does not skip:
 * on the web it is the key that moves between the form's controls, and taking
 * it away would strand anyone not using a mouse — Skip is a button instead.
 * And there is no match-type choice, because `apply_review` writes `contains`
 * and the apply route has no field for anything else; the form says so rather
 * than offering a control that could not be honoured.
 */
@customElement('nigel-review-screen')
export class NigelReviewScreen extends LitElement {
  static styles = css`
    :host {
      display: grid;
      gap: var(--wa-space-m, 12px);
      align-content: start;
      padding: var(--wa-space-l, 16px);
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
      max-width: 52rem;
    }

    .summary-counts {
      display: grid;
      grid-template-columns: max-content 1fr;
      gap: var(--wa-space-2xs, 4px) var(--wa-space-m, 12px);
      margin: 0;
    }

    .summary-counts dt {
      color: var(--wa-color-muted);
    }

    .summary-counts dd {
      margin: 0;
      font-weight: var(--wa-font-weight-medium, 500);
    }

    a {
      color: var(--wa-color-brand);
    }

    button.undo-last {
      font: inherit;
      padding: var(--wa-space-xs, 6px) var(--wa-space-m, 12px);
      border-radius: var(--wa-radius-m, 8px);
      border: 1px solid var(--wa-color-border);
      background: var(--wa-color-surface);
      color: inherit;
      cursor: pointer;
    }

    button.undo-last:disabled {
      opacity: 0.5;
      cursor: default;
    }
  `;

  /** Supplied by the registry from the screen context. */
  @property({ attribute: false })
  client!: ApiClient;

  @property({ attribute: false })
  params: URLSearchParams = new URLSearchParams();

  @property({ attribute: false })
  navigate?: (screen: ScreenId, params?: URLSearchParams) => void;

  @state() private phase: Phase = 'loading';
  @state() private queue: ReviewItem[] = [];
  @state() private index = 0;
  @state() private categories: CategoryRow[] = [];
  @state() private busy = false;
  @state() private formError: string | null = null;
  @state() private loadError: string | null = null;

  @state() private ruleTest: RuleTestResult | null = null;
  @state() private ruleTestBusy = false;
  @state() private ruleTestError: string | null = null;

  @query('wc-review-form') private form?: WcReviewForm;

  /** The decision stack, one entry per transaction left behind. */
  private history: Decision[] = [];

  /** The request the loaded queue answers, so a re-render cannot refetch. */
  private loadedKey: string | null = null;

  /** Drops a rule-test response whose pattern is no longer the current one. */
  private ruleTestSeq = 0;
  private ruleTestTimer: ReturnType<typeof setTimeout> | null = null;

  connectedCallback(): void {
    super.connectedCallback();
    this.addEventListener('keydown', this.handleKeydown);
  }

  disconnectedCallback(): void {
    super.disconnectedCallback();
    this.removeEventListener('keydown', this.handleKeydown);
    if (this.ruleTestTimer !== null) clearTimeout(this.ruleTestTimer);
  }

  willUpdate(changed: PropertyValues<this>): void {
    if (!changed.has('params')) return;

    const key = String(singleIdFrom(this.params) ?? 'queue');
    if (key === this.loadedKey) return;
    this.loadedKey = key;
    void this.load();
  }

  // -- loading --------------------------------------------------------------

  private async load(): Promise<void> {
    this.phase = 'loading';
    this.loadError = null;
    this.history = [];
    this.index = 0;

    const singleId = singleIdFrom(this.params);

    try {
      const [items, categories] = await Promise.all([
        singleId === null
          ? this.client.getReviewQueue()
          : this.client.getReviewTransaction(singleId).then((row) => [row]),
        this.client.getCategories(),
      ]);

      this.queue = items.map(toReviewItem);
      this.categories = categories;
      this.phase = this.queue.length === 0 ? 'empty' : 'reviewing';
    } catch (error) {
      this.queue = [];
      this.phase = 'error';
      this.loadError =
        singleId !== null && error instanceof ApiError && error.status === 404
          ? `Transaction ${singleId} is not there any more.`
          : error instanceof ApiError
            ? error.message
            : 'Could not load the review queue.';

      // A locked or expired session is the shell's story to tell.
      if (!(error instanceof ApiError) || (!error.isLocked && !error.isUnauthorized)) {
        dispatchNcToast(this, { message: this.loadError, variant: 'danger' });
      }
    }
  }

  // -- the review loop ------------------------------------------------------

  private get current(): ReviewItem | undefined {
    return this.queue[this.index];
  }

  private get categoryOptions(): CategoryOption[] {
    return this.categories.map((category) => ({
      id: category.id,
      name: category.name,
      categoryType: category.categoryType,
    }));
  }

  /** Move on, resetting the form and finishing when the queue runs out. */
  private advance(): void {
    this.index += 1;
    this.formError = null;
    this.clearRuleTest();
    this.form?.reset();

    if (this.index >= this.queue.length) this.phase = 'summary';
  }

  private handleApply = async (event: Event): Promise<void> => {
    const detail = (event as CustomEvent<NcReviewApplyDetail>).detail;
    const item = this.current;
    if (!item || this.busy) return;

    this.busy = true;
    this.formError = null;

    try {
      const response = await this.client.applyReview(item.id, {
        categoryId: detail.categoryId,
        ...(detail.vendor === null ? {} : { vendor: detail.vendor }),
        ...(detail.createRule
          ? { createRule: true, rulePattern: detail.rulePattern ?? '' }
          : {}),
      });
      this.history.push({
        transactionId: item.id,
        ruleId: response.ruleId,
      });
      this.advance();
    } catch (error) {
      this.handleApplyFailure(error);
    } finally {
      this.busy = false;
    }
  };

  /**
   * A transaction that is gone is not a reason to wedge the queue — another
   * tab, or an undone import, can take one out from under a review that is
   * already open. It counts as a skip so that stepping back over it does not
   * try to undo a decision that was never made.
   *
   * Every other failure, including the apply route's other 404 — a category
   * that has been deleted since the picker listed it — holds the transaction on
   * screen. Moving on from that one would file it as reviewed and leave it
   * uncategorized.
   */
  private handleApplyFailure(error: unknown): void {
    if (isMissingTransaction(error)) {
      dispatchNcToast(this, {
        message: 'That transaction is gone — moving on.',
        variant: 'info',
      });
      this.history.push(null);
      this.advance();
      return;
    }

    this.formError =
      error instanceof ApiError ? error.message : 'Could not apply that decision.';

    if (!(error instanceof ApiError) || (!error.isLocked && !error.isUnauthorized)) {
      dispatchNcToast(this, { message: this.formError, variant: 'danger' });
    }
  }

  /** Leave it flagged and move on: no request, so nothing can have changed. */
  private handleSkip = (): void => {
    if (this.busy || !this.current) return;
    this.history.push(null);
    this.advance();
  };

  private handleBack = async (): Promise<void> => {
    if (this.busy || this.index === 0) return;

    const decision = this.history.pop();
    if (decision === undefined) return;

    this.index -= 1;
    this.phase = 'reviewing';
    this.formError = null;
    this.clearRuleTest();
    this.form?.reset();

    if (decision === null) return;

    this.busy = true;
    try {
      const restored = await this.client.undoReview(decision.transactionId, {
        ...(decision.ruleId === null ? {} : { ruleId: decision.ruleId }),
      });
      this.queue = this.queue.map((item) =>
        item.id === restored.id ? toReviewItem(restored) : item,
      );
    } catch (error) {
      // Put the stack back exactly as it was: a decision the server still
      // holds must stay on the stack, or a later Back would skip past it.
      this.history.push(decision);
      this.index += 1;
      if (this.index >= this.queue.length) this.phase = 'summary';

      const message =
        error instanceof ApiError ? error.message : 'Could not undo that decision.';
      dispatchNcToast(this, { message, variant: 'danger' });
    } finally {
      this.busy = false;
    }
  };

  /** Esc is Back, as in the TUI. Enter is the form's own submit. */
  private handleKeydown = (event: KeyboardEvent): void => {
    if (event.key !== 'Escape' || this.phase !== 'reviewing') return;
    if (this.index === 0) return;
    event.preventDefault();
    void this.handleBack();
  };

  // -- the rule test preview ------------------------------------------------

  private clearRuleTest(): void {
    if (this.ruleTestTimer !== null) clearTimeout(this.ruleTestTimer);
    this.ruleTestTimer = null;
    this.ruleTestSeq += 1;
    this.ruleTest = null;
    this.ruleTestError = null;
    this.ruleTestBusy = false;
  }

  private handlePatternChange = (event: Event): void => {
    const pattern = (event as CustomEvent<NcRulePatternChangeDetail>).detail.pattern;

    if (this.ruleTestTimer !== null) clearTimeout(this.ruleTestTimer);
    this.ruleTestSeq += 1;

    if (pattern.trim() === '') {
      this.ruleTest = null;
      this.ruleTestError = null;
      this.ruleTestBusy = false;
      return;
    }

    this.ruleTestBusy = true;
    this.ruleTestError = null;
    const seq = this.ruleTestSeq;
    this.ruleTestTimer = setTimeout(() => {
      void this.runRuleTest(pattern, seq);
    }, RULE_TEST_DEBOUNCE_MS);
  };

  private async runRuleTest(pattern: string, seq: number): Promise<void> {
    try {
      // `contains` is what the apply route will actually save, so testing
      // anything else would preview a rule that is not the one being made.
      const result = await this.client.testRule({ pattern, matchType: 'contains' });
      if (seq !== this.ruleTestSeq) return;
      this.ruleTest = result;
    } catch (error) {
      if (seq !== this.ruleTestSeq) return;
      this.ruleTest = null;
      this.ruleTestError =
        error instanceof ApiError ? error.message : 'Could not test that pattern.';
    } finally {
      if (seq === this.ruleTestSeq) this.ruleTestBusy = false;
    }
  }

  // -- rendering ------------------------------------------------------------

  render() {
    switch (this.phase) {
      case 'loading':
        return html`<wc-spinner size="l" label="Loading the review queue" show-label></wc-spinner>`;
      case 'error':
        return html`
          <wc-empty-state
            icon="wc-icon-review"
            heading="Nothing to review here"
            message=${this.loadError ?? 'Could not load the review queue.'}
          >
            <a slot="actions" href="#/register">Open the register</a>
          </wc-empty-state>
        `;
      case 'empty':
        return html`
          <wc-empty-state
            icon="wc-icon-review"
            heading="Nothing to review"
            message="Every transaction has a category. Importing a statement is what puts new ones here."
          >
            <a slot="actions" href="#/register">Open the register</a>
          </wc-empty-state>
        `;
      case 'summary':
        return this.renderSummary();
      case 'reviewing':
        return this.renderReviewing();
    }
  }

  private renderReviewing(): TemplateResult {
    const item = this.current;
    if (!item) return html`${nothing}`;

    const counts = summarize(this.history);

    return html`
      <wc-review-progress
        .current=${this.index + 1}
        .total=${this.queue.length}
        .reviewed=${counts.reviewed}
        .skipped=${counts.skipped}
      ></wc-review-progress>

      <wc-review-card
        date=${item.date}
        description=${item.description}
        .amount=${item.amount}
        account-name=${item.accountName}
        .currentCategory=${item.category}
        .currentVendor=${item.vendor}
      ></wc-review-card>

      <wc-review-form
        .categories=${this.categoryOptions}
        description-for-pattern=${item.description}
        ?busy=${this.busy}
        ?can-go-back=${this.index > 0}
        .error=${this.formError}
        @nc-review-apply=${this.handleApply}
        @nc-review-skip=${this.handleSkip}
        @nc-review-back=${this.handleBack}
        @nc-rule-pattern-change=${this.handlePatternChange}
      >
        <wc-rule-test-preview
          slot="rule-test"
          ?busy=${this.ruleTestBusy}
          .result=${this.ruleTest}
          .error=${this.ruleTestError}
        ></wc-rule-test-preview>
      </wc-review-form>
    `;
  }

  private renderSummary(): TemplateResult {
    const counts = summarize(this.history);
    const single = this.queue.length === 1;

    return html`
      <wc-panel
        heading=${single ? 'Transaction reviewed' : 'Review complete'}
        description=${single
          ? 'That is the one you asked for.'
          : 'Everything in the queue has been through.'}
      >
        <dl class="summary-counts">
          <dt>Reviewed</dt>
          <dd>${counts.reviewed}</dd>
          <dt>Skipped</dt>
          <dd>${counts.skipped}</dd>
          <dt>Rules created</dt>
          <dd>${counts.rulesCreated}</dd>
        </dl>
        ${this.history.length > 0
          ? html`<button
              class="undo-last"
              slot="actions"
              type="button"
              ?disabled=${this.busy}
              @click=${this.handleBack}
            >
              Back to the last transaction
            </button>`
          : nothing}
        <a slot="actions" href="#/register">Open the register</a>
        <a slot="actions" href="#/dashboard">Back to the dashboard</a>
      </wc-panel>
    `;
  }
}

export function renderReview(ctx: ScreenContext): TemplateResult {
  return html`
    <nigel-review-screen
      .client=${ctx.client}
      .params=${ctx.params}
      .navigate=${ctx.navigate}
    ></nigel-review-screen>
  `;
}

declare global {
  interface HTMLElementTagNameMap {
    'nigel-review-screen': NigelReviewScreen;
  }
}

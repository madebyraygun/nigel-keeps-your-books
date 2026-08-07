import { LitElement, html, css, nothing, type TemplateResult } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import '@nigel/ui';
import {
  confirmDialog,
  EMPTY_RULE_FORM,
  matchTypeLabel,
  validateRuleForm,
  type CategoryOption,
  type ManagerAction,
  type ManagerColumn,
  type ManagerRow,
  type NcManagerActionDetail,
  type NcRuleFormChangeDetail,
  type RuleFormErrors,
  type RuleFormValue,
} from '@nigel/ui';

import { ApiError, type ApiClient } from '../api/index.js';
import type { CategoryRow, RuleRow, RuleTestResult } from '../api/types.js';
import {
  asMatchType,
  categoryIdFrom,
  filterRules,
  isEmptyRulePatch,
  newRuleRequest,
  rulePatch,
  toRuleForm,
} from './rules-data.js';
import { guardrailMessage, isStaleListConflict } from './manager-errors.js';
import type { ScreenContext } from './context.js';
import type { ScreenId } from './registry.js';

/** The same window the review screen waits before asking what a pattern hits. */
const RULE_TEST_DEBOUNCE_MS = 250;

const COLUMNS: ManagerColumn[] = [
  { key: 'pattern', label: 'Pattern', mono: true },
  { key: 'matchType', label: 'Match' },
  { key: 'category', label: 'Category' },
  { key: 'vendor', label: 'Vendor' },
  { key: 'priority', label: 'Priority', align: 'end' },
  { key: 'hitCount', label: 'Hits', align: 'end' },
];

const ACTIONS: ManagerAction[] = [
  { name: 'edit', label: 'Edit', icon: 'wc-icon-edit' },
  { name: 'delete', label: 'Delete', icon: 'wc-icon-trash', variant: 'danger' },
];

interface Editor {
  /** The rule being edited; absent when creating. */
  id?: number;
  value: RuleFormValue;
}

/**
 * The rules manager, and the only place in nigel a rule can be written by hand:
 * `rules_manager.rs` lists and deletes, and `nigel rules add` is the CLI's.
 *
 * The order of the list is the semantics — priority descending, first match
 * wins — so it is never re-sorted here, and a priority edit is one of the
 * reasons every mutation refetches instead of splicing a row back in.
 */
@customElement('nigel-rules-screen')
export class NigelRulesScreen extends LitElement {
  static styles = css`
    :host {
      display: block;
    }

    .filter {
      display: flex;
      align-items: center;
      gap: var(--wa-space-s, 8px);
      font-family: var(--wa-font-family-sans);
      font-size: var(--wa-font-size-s, 13px);
      color: var(--wa-color-text);
    }

    .filter button {
      font: inherit;
      color: var(--wa-color-brand);
      background: none;
      border: 0;
      padding: 0;
      cursor: pointer;
      text-decoration: underline;
    }
  `;

  @property({ attribute: false })
  client!: ApiClient;

  @property({ attribute: false })
  params: URLSearchParams = new URLSearchParams();

  @property({ attribute: false })
  navigate?: (screen: ScreenId, params?: URLSearchParams) => void;

  @state() private rules: RuleRow[] = [];
  @state() private categories: CategoryRow[] = [];
  @state() private loading = true;
  @state() private error: string | null = null;
  @state() private retryable = false;
  @state() private editor: Editor | null = null;
  @state() private formErrors: RuleFormErrors = {};
  @state() private saving = false;
  @state() private dialogError: string | null = null;
  @state() private busyId: number | null = null;

  @state() private ruleTest: RuleTestResult | null = null;
  @state() private ruleTestBusy = false;
  @state() private ruleTestError: string | null = null;

  /** Drops a rule-test answer whose pattern is no longer the current one. */
  private ruleTestSeq = 0;
  private ruleTestTimer: ReturnType<typeof setTimeout> | null = null;

  firstUpdated(): void {
    void this.load();
  }

  disconnectedCallback(): void {
    super.disconnectedCallback();
    if (this.ruleTestTimer !== null) clearTimeout(this.ruleTestTimer);
  }

  private async load(): Promise<void> {
    this.loading = true;
    try {
      const [rules, categories] = await Promise.all([
        this.client.getRules(),
        this.client.getCategories(),
      ]);
      this.rules = rules;
      this.categories = categories;
      this.error = null;
      this.retryable = false;
    } catch (error) {
      this.rules = [];
      this.retryable = true;
      this.error =
        error instanceof ApiError ? error.message : 'Could not load the rules.';
    } finally {
      this.loading = false;
    }
  }

  /**
   * `#/rules?categoryId=12`, applied to the list already in hand — the rules
   * endpoint has no category filter, and a screen may not invent one.
   */
  private get categoryFilter(): number | null {
    return categoryIdFrom(this.params);
  }

  private get visibleRules(): RuleRow[] {
    return filterRules(this.rules, this.categoryFilter);
  }

  private get categoryOptions(): CategoryOption[] {
    return this.categories.map((category) => ({
      id: category.id,
      name: category.name,
      categoryType: category.categoryType,
    }));
  }

  private get rows(): ManagerRow[] {
    return this.visibleRules.map((rule) => ({
      id: rule.id,
      label: rule.pattern,
      cells: [
        rule.pattern,
        matchTypeLabel(rule.matchType),
        rule.category,
        rule.vendor,
        rule.priority,
        rule.hitCount,
      ],
    }));
  }

  // -- editing --------------------------------------------------------------

  private openCreate = (): void => {
    this.editor = { value: EMPTY_RULE_FORM };
    this.formErrors = {};
    this.dialogError = null;
    this.clearRuleTest();
  };

  private closeEditor = (): void => {
    this.editor = null;
    this.formErrors = {};
    this.dialogError = null;
    this.clearRuleTest();
  };

  private handleFormChange = (event: Event): void => {
    const editor = this.editor;
    if (!editor) return;

    const next = (event as CustomEvent<NcRuleFormChangeDetail>).detail.value;
    const patternChanged = next.pattern !== editor.value.pattern;
    const matchTypeChanged = next.matchType !== editor.value.matchType;
    this.editor = { ...editor, value: next };

    if (patternChanged) this.scheduleRuleTest(next, RULE_TEST_DEBOUNCE_MS);
    // Picking a match type is a click, not typing. Waiting a quarter second
    // after a discrete choice reads as a bug rather than as patience.
    else if (matchTypeChanged) this.scheduleRuleTest(next, 0);
  };

  private handleAction = (event: Event): void => {
    const { action, id } = (event as CustomEvent<NcManagerActionDetail>).detail;
    const rule = this.rules.find((candidate) => candidate.id === id);
    if (!rule) return;

    if (action === 'edit') {
      this.editor = { id, value: toRuleForm(rule) };
      this.formErrors = {};
      this.dialogError = null;
      this.clearRuleTest();
      return;
    }
    if (action === 'delete') void this.confirmDelete(rule);
  };

  private handleSave = async (): Promise<void> => {
    const editor = this.editor;
    if (!editor || this.saving) return;

    const errors = validateRuleForm(editor.value);
    this.formErrors = errors;
    if (Object.keys(errors).length > 0) return;

    const current =
      editor.id === undefined
        ? undefined
        : this.rules.find((rule) => rule.id === editor.id);

    if (current) {
      const patch = rulePatch(current, editor.value);
      if (isEmptyRulePatch(patch)) {
        this.closeEditor();
        return;
      }
      await this.send(() => this.client.updateRule(current.id, patch));
      return;
    }

    await this.send(() => this.client.createRule(newRuleRequest(editor.value)));
  };

  private async send(request: () => Promise<unknown>): Promise<void> {
    this.saving = true;
    this.dialogError = null;
    try {
      await request();
      this.closeEditor();
      await this.load();
    } catch (error) {
      this.dialogError = guardrailMessage(error, 'rule');

      // A category that went away under the form: the picker must stop
      // offering it, so reload the chart of accounts behind the dialog.
      if (error instanceof ApiError && error.status === 404) {
        this.categories = await this.client.getCategories().catch(() => this.categories);
      }
    } finally {
      this.saving = false;
    }
  }

  // -- deleting -------------------------------------------------------------

  private async confirmDelete(rule: RuleRow): Promise<void> {
    const confirmed = await confirmDialog({
      heading: 'Delete rule',
      message: `Delete the rule for “${rule.pattern}”? It stops matching new transactions; anything it already categorized keeps its category.`,
      confirmLabel: 'Delete',
      variant: 'danger',
    });
    if (!confirmed) return;

    this.busyId = rule.id;
    try {
      await this.client.deleteRule(rule.id);
      this.error = null;
      await this.load();
    } catch (error) {
      const message = guardrailMessage(error, 'rule');
      // An already-deleted rule means this list is stale, and the message says
      // it has been refreshed — so refresh it first, since a successful load
      // clears the error region it is about to be written to.
      if (isStaleListConflict(error)) await this.load();
      this.error = message;
      this.retryable = false;
    } finally {
      this.busyId = null;
    }
  }

  private handleErrorAction = (): void => {
    if (this.retryable) void this.load();
  };

  private handleErrorDismiss = (): void => {
    this.error = null;
  };

  private clearFilter = (): void => {
    this.navigate?.('rules');
  };

  // -- the live pattern test ------------------------------------------------

  private clearRuleTest(): void {
    if (this.ruleTestTimer !== null) clearTimeout(this.ruleTestTimer);
    this.ruleTestTimer = null;
    this.ruleTestSeq += 1;
    this.ruleTest = null;
    this.ruleTestError = null;
    this.ruleTestBusy = false;
  }

  private scheduleRuleTest(value: RuleFormValue, delay: number): void {
    if (this.ruleTestTimer !== null) clearTimeout(this.ruleTestTimer);
    this.ruleTestSeq += 1;

    // The route's guard is `pattern.is_empty()`, not a trim, so whitespace
    // would be scanned against every description in the database for nothing.
    if (value.pattern.trim() === '') {
      this.ruleTestTimer = null;
      this.ruleTest = null;
      this.ruleTestError = null;
      this.ruleTestBusy = false;
      return;
    }

    this.ruleTestBusy = true;
    this.ruleTestError = null;
    const seq = this.ruleTestSeq;
    this.ruleTestTimer = setTimeout(() => {
      void this.runRuleTest(value, seq);
    }, delay);
  }

  private async runRuleTest(value: RuleFormValue, seq: number): Promise<void> {
    try {
      const result = await this.client.testRule({
        pattern: value.pattern,
        matchType: asMatchType(value.matchType) ?? 'contains',
      });
      if (seq !== this.ruleTestSeq) return;
      this.ruleTest = result;
    } catch (error) {
      if (seq !== this.ruleTestSeq) return;
      this.ruleTest = null;
      // An uncompilable regex, most of the time. The form still saves: the
      // server is the authority on both, and it will say the same thing there.
      this.ruleTestError =
        error instanceof ApiError ? error.message : 'Could not test that pattern.';
    } finally {
      if (seq === this.ruleTestSeq) this.ruleTestBusy = false;
    }
  }

  // -- rendering ------------------------------------------------------------

  render() {
    const rows = this.rows;
    const empty = !this.loading && rows.length === 0;
    const filter = this.categoryFilter;
    const filterName =
      filter === null
        ? null
        : (this.categories.find((category) => category.id === filter)?.name ??
          `category ${filter}`);

    return html`
      <wc-manager-layout
        heading="Categorization Rules"
        description="Applied top to bottom — the first match wins."
        .count=${this.loading ? null : rows.length}
        add-label="Add rule"
        ?busy=${this.loading}
        ?empty=${empty}
        .error=${this.error}
        error-action-label=${this.retryable ? 'Try again' : ''}
        @nc-manager-add=${this.openCreate}
        @nc-manager-error-action=${this.handleErrorAction}
        @nc-manager-error-dismiss=${this.handleErrorDismiss}
      >
        ${filterName === null
          ? nothing
          : html`
              <p class="filter" slot="toolbar" data-filter>
                <span>Showing rules for ${filterName}</span>
                <button type="button" @click=${this.clearFilter}>Show all</button>
              </p>
            `}

        <wc-manager-table
          caption="Categorization rules"
          .columns=${COLUMNS}
          .rows=${rows}
          .actions=${ACTIONS}
          .busyId=${this.busyId}
          @nc-manager-action=${this.handleAction}
        ></wc-manager-table>

        <wc-empty-state
          slot="empty"
          icon="wc-icon-rule"
          heading=${filterName === null ? 'No rules yet' : 'No rules for that category'}
          message=${filterName === null
            ? 'Rules categorize future imports automatically. Create one here, or from the review screen.'
            : 'Nothing here assigns that category.'}
        >
          ${filterName === null
            ? html`<a slot="actions" href="#/review">Go to review</a>`
            : html`<a slot="actions" href="#/rules">Show all rules</a>`}
        </wc-empty-state>

        ${this.renderEditor()}
      </wc-manager-layout>
    `;
  }

  private renderEditor(): TemplateResult | typeof nothing {
    const editor = this.editor;
    if (!editor) return nothing;

    const creating = editor.id === undefined;

    return html`
      <wc-manager-dialog
        slot="overlay"
        open
        heading=${creating ? 'Add rule' : 'Edit rule'}
        confirm-label=${creating ? 'Add rule' : 'Save'}
        ?busy=${this.saving}
        .error=${this.dialogError}
        @nc-manager-save=${this.handleSave}
        @nc-manager-cancel=${this.closeEditor}
      >
        <wc-rule-form
          .value=${editor.value}
          .errors=${this.formErrors}
          .categories=${this.categoryOptions}
          ?disabled=${this.saving}
          @nc-rule-form-change=${this.handleFormChange}
        >
          <wc-rule-test-preview
            slot="test"
            ?busy=${this.ruleTestBusy}
            .result=${this.ruleTest}
            .error=${this.ruleTestError}
          ></wc-rule-test-preview>
        </wc-rule-form>
      </wc-manager-dialog>
    `;
  }
}

export function renderRules(ctx: ScreenContext): TemplateResult {
  return html`
    <nigel-rules-screen
      .client=${ctx.client}
      .params=${ctx.params}
      .navigate=${ctx.navigate}
    ></nigel-rules-screen>
  `;
}

declare global {
  interface HTMLElementTagNameMap {
    'nigel-rules-screen': NigelRulesScreen;
  }
}

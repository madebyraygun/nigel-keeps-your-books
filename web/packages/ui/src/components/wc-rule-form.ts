import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/select/select.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import './wc-category-picker.js';
import type { CategoryOption } from './category-option.js';
import type { NcCategoryChangeDetail } from './wc-category-picker.js';

/** The match types the categorizer understands, in `cli/rules.rs` order. */
export const MATCH_TYPES = ['contains', 'starts_with', 'regex'] as const;

export type MatchTypeValue = (typeof MATCH_TYPES)[number];

const MATCH_TYPE_LABELS: Record<string, string> = {
  contains: 'Contains',
  starts_with: 'Starts with',
  regex: 'Regular expression',
};

/** An out-of-vocabulary match type reads as itself rather than as a guess. */
export function matchTypeLabel(value: string): string {
  return MATCH_TYPE_LABELS[value] ?? value;
}

export interface RuleFormValue {
  pattern: string;
  matchType: string;
  categoryId: number | null;
  vendor: string;
  priority: number;
}

export interface RuleFormErrors {
  pattern?: string;
  categoryId?: string;
}

export interface NcRuleFormChangeDetail {
  value: RuleFormValue;
}

export const EMPTY_RULE_FORM: RuleFormValue = {
  pattern: '',
  matchType: 'contains',
  categoryId: null,
  vendor: '',
  priority: 0,
};

export function validateRuleForm(value: RuleFormValue): RuleFormErrors {
  const errors: RuleFormErrors = {};
  if (value.pattern.trim() === '') errors.pattern = 'Pattern is required';
  if (value.categoryId === null) errors.categoryId = 'Choose a category';
  return errors;
}

/**
 * The rule add/edit field group, and the web's only way to write a rule —
 * `rules_manager.rs` lists and deletes, so this is where the TUI's gap is.
 *
 * The category field is `wc-category-picker` rather than a `wa-select`: this
 * Web Awesome build has no option groups, and income-then-expense grouping is
 * how every other picker in the app reads.
 *
 * There is no regex validation here on purpose. JavaScript's `RegExp` and the
 * Rust `regex` crate accept different languages — lookahead and backreferences
 * compile in one and are refused by the other — so a local check would be wrong
 * in both directions. The pattern preview and the save both ask the server.
 */
@customElement('wc-rule-form')
export class WcRuleForm extends LitElement {
  static styles = css`
    :host {
      display: block;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    .fields {
      display: grid;
      gap: var(--wa-space-m, 12px);
      min-width: 24rem;
    }

    .row {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: var(--wa-space-m, 12px);
    }

    .error {
      margin: var(--wa-space-2xs, 4px) 0 0;
      color: var(--wa-color-danger, #b3261e);
      font-size: var(--wa-font-size-s, 13px);
    }

    .hint {
      margin: var(--wa-space-2xs, 4px) 0 0;
      color: var(--wa-color-muted);
      font-size: var(--wa-font-size-s, 13px);
    }

    .test {
      /* The panel lists every matching description; the dialog must not grow
         without limit because a pattern turned out to be broad. */
      max-height: 12rem;
      overflow-y: auto;
    }
  `;

  @property({ attribute: false })
  value: RuleFormValue = EMPTY_RULE_FORM;

  @property({ attribute: false })
  errors: RuleFormErrors = {};

  @property({ attribute: false })
  categories: CategoryOption[] = [];

  @property({ type: Boolean })
  disabled = false;

  private emit(next: Partial<RuleFormValue>): void {
    this.dispatchEvent(
      new CustomEvent<NcRuleFormChangeDetail>('nc-rule-form-change', {
        detail: { value: { ...this.value, ...next } },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private handlePattern = (event: Event) => {
    this.emit({ pattern: (event.target as HTMLInputElement).value });
  };

  private handleMatchType = (event: Event) => {
    this.emit({ matchType: String((event.target as HTMLInputElement).value) });
  };

  private handleVendor = (event: Event) => {
    this.emit({ vendor: (event.target as HTMLInputElement).value });
  };

  private handlePriority = (event: Event) => {
    const raw = (event.target as HTMLInputElement).value;
    const parsed = Number.parseInt(raw, 10);
    this.emit({ priority: Number.isNaN(parsed) ? 0 : parsed });
  };

  private handleCategory = (event: Event) => {
    const detail = (event as CustomEvent<NcCategoryChangeDetail>).detail;
    this.emit({ categoryId: detail.categoryId });
  };

  render() {
    const unknownMatchType = !MATCH_TYPES.includes(this.value.matchType as MatchTypeValue);

    return html`
      <div class="fields">
        <div>
          <wa-input
            data-pattern
            label="Pattern"
            autocomplete="off"
            value=${this.value.pattern}
            ?disabled=${this.disabled}
            @input=${this.handlePattern}
          ></wa-input>
          ${this.errors.pattern
            ? html`<p class="error" role="alert">${this.errors.pattern}</p>`
            : nothing}
        </div>

        <div>
          <wa-select
            data-match-type
            label="Match type"
            value=${this.value.matchType}
            ?disabled=${this.disabled}
            @change=${this.handleMatchType}
          >
            ${MATCH_TYPES.map(
              (type) =>
                html`<wa-option value=${type}>${matchTypeLabel(type)}</wa-option>`,
            )}
            ${unknownMatchType
              ? html`<wa-option value=${this.value.matchType}
                  >${this.value.matchType}</wa-option
                >`
              : nothing}
          </wa-select>
          <p class="hint">
            Contains and starts-with ignore case; a regular expression is
            case-sensitive.
          </p>
        </div>

        <div>
          <wc-category-picker
            data-category
            .options=${this.categories}
            .value=${this.value.categoryId}
            ?disabled=${this.disabled}
            ?invalid=${this.errors.categoryId !== undefined}
            @nc-category-change=${this.handleCategory}
          ></wc-category-picker>
          ${this.errors.categoryId
            ? html`<p class="error" role="alert">${this.errors.categoryId}</p>`
            : nothing}
        </div>

        <div class="row">
          <wa-input
            data-vendor
            label="Vendor"
            autocomplete="off"
            hint="Optional."
            value=${this.value.vendor}
            ?disabled=${this.disabled}
            @input=${this.handleVendor}
          ></wa-input>
          <div>
            <wa-input
              data-priority
              label="Priority"
              type="number"
              step="1"
              value=${String(this.value.priority)}
              ?disabled=${this.disabled}
              @input=${this.handlePriority}
            ></wa-input>
            <p class="hint">Higher runs first; the first match wins.</p>
          </div>
        </div>

        <div class="test"><slot name="test"></slot></div>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-rule-form': WcRuleForm;
  }
  interface HTMLElementEventMap {
    'nc-rule-form-change': CustomEvent<NcRuleFormChangeDetail>;
  }
}

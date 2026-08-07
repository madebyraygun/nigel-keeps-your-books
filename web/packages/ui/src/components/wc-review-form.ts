import { LitElement, html, css, nothing } from 'lit';
import { customElement, property, state, query } from 'lit/decorators.js';
import './wc-category-picker.js';
import type { WcCategoryPicker, NcCategoryChangeDetail } from './wc-category-picker.js';
import type { CategoryOption } from './category-option.js';

export interface NcReviewApplyDetail {
  categoryId: number;
  /** Null when the field was left empty — there is no vendor to record. */
  vendor: string | null;
  createRule: boolean;
  /** The pattern, only when `createRule` is true. */
  rulePattern: string | null;
}

export interface NcRulePatternChangeDetail {
  pattern: string;
}

/**
 * The pattern the TUI offers when you ask it to build a rule: the first two
 * words of the description.
 *
 * Two words rather than the whole description because a bank line ends in a
 * transaction id that will never repeat — `ADOBE CREATIVE` matches next month,
 * `ADOBE CREATIVE CLOUD 0000123456` matches nothing ever again.
 */
export function patternPrefill(description: string): string {
  const words = description.trim().split(/\s+/).filter(Boolean);
  return words.slice(0, 2).join(' ');
}

/**
 * The review decision: a category, an optional vendor, and an optional rule.
 *
 * Controlled and inert — it holds its own field values and emits, and the
 * screen above it owns every network call, including the debounced rule test
 * whose result arrives back through the `rule-test` slot.
 *
 * Apply is a real submit button in a real form, so Enter applies without a key
 * handler and the browser's own focus order does the rest. Skip and Back are
 * ordinary buttons: the TUI skips on Tab, which on the web is the key that
 * moves between these five controls and cannot be taken away from it.
 */
@customElement('wc-review-form')
export class WcReviewForm extends LitElement {
  static styles = css`
    :host {
      display: block;
      background: var(--wa-color-surface);
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-l, 12px);
      padding: var(--wa-space-l, 16px);
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    .fields {
      display: grid;
      gap: var(--wa-space-m, 12px);
    }

    label {
      display: block;
      font-size: var(--wa-font-size-s, 13px);
      font-weight: var(--wa-font-weight-medium, 500);
      margin-bottom: var(--wa-space-2xs, 4px);
    }

    input[type='text'] {
      width: 100%;
      box-sizing: border-box;
      font: inherit;
      color: inherit;
      background: var(--wa-color-surface);
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-m, 8px);
      padding: var(--wa-space-xs, 6px) var(--wa-space-s, 8px);
    }

    input[type='text']:focus-visible {
      outline: 2px solid var(--wa-color-brand);
      outline-offset: 1px;
    }

    .checkbox {
      display: flex;
      align-items: center;
      gap: var(--wa-space-xs, 6px);
    }

    .checkbox label {
      margin: 0;
      font-weight: var(--wa-font-weight-normal, 400);
    }

    .rule {
      display: grid;
      gap: var(--wa-space-s, 8px);
      padding-left: var(--wa-space-m, 12px);
      border-left: 2px solid var(--wa-color-border);
    }

    .hint {
      margin: 0;
      font-size: var(--wa-font-size-s, 13px);
      color: var(--wa-color-muted);
    }

    .field-error {
      margin: var(--wa-space-2xs, 4px) 0 0;
      font-size: var(--wa-font-size-s, 13px);
      color: var(--wa-color-danger, #b3261e);
    }

    .form-error {
      margin: 0 0 var(--wa-space-m, 12px);
      padding: var(--wa-space-s, 8px) var(--wa-space-m, 12px);
      border-radius: var(--wa-radius-m, 8px);
      background: var(--wa-color-danger-fill, rgb(179 38 30 / 10%));
      color: var(--wa-color-danger, #b3261e);
      font-size: var(--wa-font-size-s, 13px);
    }

    .actions {
      display: flex;
      gap: var(--wa-space-s, 8px);
      margin-top: var(--wa-space-l, 16px);
      flex-wrap: wrap;
    }

    button {
      font: inherit;
      padding: var(--wa-space-xs, 6px) var(--wa-space-m, 12px);
      border-radius: var(--wa-radius-m, 8px);
      border: 1px solid var(--wa-color-border);
      background: var(--wa-color-surface);
      color: inherit;
      cursor: pointer;
    }

    button[type='submit'] {
      background: var(--wa-color-brand);
      border-color: var(--wa-color-brand);
      color: var(--wa-color-on-brand, #fff);
      font-weight: var(--wa-font-weight-medium, 500);
    }

    button:disabled {
      opacity: 0.5;
      cursor: default;
    }

    .keys {
      margin: var(--wa-space-s, 8px) 0 0;
      font-size: var(--wa-font-size-s, 13px);
      color: var(--wa-color-muted);
    }

    kbd {
      font-family: var(--wa-font-family-mono, monospace);
    }
  `;

  @property({ attribute: false })
  categories: CategoryOption[] = [];

  /** The description the rule pattern is prefilled from. */
  @property({ type: String, attribute: 'description-for-pattern' })
  descriptionForPattern = '';

  @property({ type: Boolean })
  busy = false;

  /** A server-side rejection, shown above the fields. */
  @property({ type: String })
  error: string | null = null;

  @property({ type: Boolean, attribute: 'can-go-back' })
  canGoBack = false;

  /** Open the rule half — the previews need it, and so does a test. */
  @property({ type: Boolean, attribute: 'create-rule' })
  createRule = false;

  @property({ type: String, attribute: 'rule-pattern' })
  rulePattern = '';

  @state() private categoryId: number | null = null;
  @state() private vendor = '';
  @state() private showErrors = false;

  @query('wc-category-picker') private picker?: WcCategoryPicker;

  /** Clear every field for the next transaction. */
  reset(): void {
    this.categoryId = null;
    this.vendor = '';
    this.createRule = false;
    this.rulePattern = '';
    this.showErrors = false;
    this.error = null;
    this.picker?.reset();
  }

  /** Put the cursor where the TUI puts it: the category filter. */
  override focus(): void {
    this.picker?.focus();
  }

  private get categoryMissing(): boolean {
    return this.categoryId === null;
  }

  private get patternMissing(): boolean {
    return this.createRule && this.rulePattern.trim() === '';
  }

  private handleCategoryChange = (event: Event): void => {
    this.categoryId = (event as CustomEvent<NcCategoryChangeDetail>).detail.categoryId;
  };

  private handleVendorInput(event: Event): void {
    this.vendor = (event.target as HTMLInputElement).value;
  }

  private handleCreateRuleChange(event: Event): void {
    this.createRule = (event.target as HTMLInputElement).checked;

    if (this.createRule && this.rulePattern === '') {
      this.rulePattern = patternPrefill(this.descriptionForPattern);
    }
    // Tell the screen either way: switching the rule off should retract the
    // preview, not leave the last pattern's matches sitting under a closed
    // section.
    this.emitPattern(this.createRule ? this.rulePattern : '');
  }

  private handlePatternInput(event: Event): void {
    this.rulePattern = (event.target as HTMLInputElement).value;
    this.emitPattern(this.rulePattern);
  }

  private emitPattern(pattern: string): void {
    this.dispatchEvent(
      new CustomEvent<NcRulePatternChangeDetail>('nc-rule-pattern-change', {
        detail: { pattern },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private handleSubmit(event: Event): void {
    event.preventDefault();
    if (this.busy) return;

    if (this.categoryMissing || this.patternMissing) {
      this.showErrors = true;
      return;
    }

    const vendor = this.vendor.trim();
    this.dispatchEvent(
      new CustomEvent<NcReviewApplyDetail>('nc-review-apply', {
        detail: {
          categoryId: this.categoryId as number,
          vendor: vendor === '' ? null : vendor,
          createRule: this.createRule,
          rulePattern: this.createRule ? this.rulePattern.trim() : null,
        },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private emit(name: 'nc-review-skip' | 'nc-review-back'): void {
    this.dispatchEvent(new CustomEvent(name, { bubbles: true, composed: true }));
  }

  render() {
    return html`
      ${this.error ? html`<p class="form-error" role="alert">${this.error}</p>` : nothing}
      <form @submit=${this.handleSubmit} novalidate>
        <div class="fields">
          <div>
            <wc-category-picker
              .options=${this.categories}
              .value=${this.categoryId}
              ?disabled=${this.busy}
              ?invalid=${this.showErrors && this.categoryMissing}
              @nc-category-change=${this.handleCategoryChange}
            ></wc-category-picker>
            ${this.showErrors && this.categoryMissing
              ? html`<p class="field-error">Pick a category before applying.</p>`
              : nothing}
          </div>

          <div>
            <label for="vendor">Vendor (optional)</label>
            <input
              id="vendor"
              type="text"
              .value=${this.vendor}
              ?disabled=${this.busy}
              @input=${this.handleVendorInput}
            />
          </div>

          <div class="checkbox">
            <input
              id="create-rule"
              type="checkbox"
              .checked=${this.createRule}
              ?disabled=${this.busy}
              @change=${this.handleCreateRuleChange}
            />
            <label for="create-rule">Create a rule for future matches</label>
          </div>

          ${this.createRule ? this.renderRule() : nothing}
        </div>

        <div class="actions">
          <button type="submit" ?disabled=${this.busy}>Apply</button>
          <button type="button" ?disabled=${this.busy} @click=${() => this.emit('nc-review-skip')}>
            Skip
          </button>
          <button
            type="button"
            ?disabled=${this.busy || !this.canGoBack}
            @click=${() => this.emit('nc-review-back')}
          >
            Back
          </button>
        </div>
        <p class="keys"><kbd>Enter</kbd> apply · <kbd>Esc</kbd> back</p>
      </form>
    `;
  }

  private renderRule() {
    return html`
      <div class="rule">
        <div>
          <label for="rule-pattern">Pattern</label>
          <input
            id="rule-pattern"
            type="text"
            .value=${this.rulePattern}
            ?disabled=${this.busy}
            aria-describedby="rule-hint"
            @input=${this.handlePatternInput}
          />
          ${this.showErrors && this.patternMissing
            ? html`<p class="field-error">A rule needs a pattern.</p>`
            : nothing}
        </div>
        <p class="hint" id="rule-hint">
          Matches any transaction whose description contains this text.
        </p>
        <slot name="rule-test"></slot>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-review-form': WcReviewForm;
  }
}

import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import './wc-money.js';

/**
 * The transaction being reviewed.
 *
 * Presentation only — it emits nothing, because the decision belongs to the
 * form below it. The four facts the TUI prints in its detail pane are the four
 * facts here, with the description and the amount carrying the visual weight:
 * they are what a decision is actually made on, and date and account are
 * context.
 *
 * A `<dl>` rather than four styled divs so a screen reader reads
 * "Description, ADOBE CREATIVE CLOUD" instead of two unrelated strings.
 */
@customElement('wc-review-card')
export class WcReviewCard extends LitElement {
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

    .headline {
      display: flex;
      align-items: baseline;
      justify-content: space-between;
      gap: var(--wa-space-m, 12px);
      flex-wrap: wrap;
    }

    .description {
      margin: 0;
      font-size: var(--wa-font-size-lg, 16px);
      font-weight: var(--wa-font-weight-semibold, 600);
      overflow-wrap: anywhere;
    }

    .amount {
      font-size: var(--wa-font-size-lg, 16px);
      font-weight: var(--wa-font-weight-semibold, 600);
      white-space: nowrap;
    }

    dl {
      display: grid;
      grid-template-columns: max-content 1fr;
      gap: var(--wa-space-2xs, 4px) var(--wa-space-m, 12px);
      margin: var(--wa-space-m, 12px) 0 0;
      font-size: var(--wa-font-size-s, 13px);
    }

    dt {
      color: var(--wa-color-muted);
    }

    dd {
      margin: 0;
      overflow-wrap: anywhere;
    }

    .current {
      margin: var(--wa-space-m, 12px) 0 0;
      padding-top: var(--wa-space-s, 8px);
      border-top: 1px dashed var(--wa-color-border);
      font-size: var(--wa-font-size-s, 13px);
      color: var(--wa-color-muted);
    }
  `;

  /** `YYYY-MM-DD`. */
  @property({ type: String })
  date = '';

  @property({ type: String })
  description = '';

  /** Negative is an expense, positive is income. */
  @property({ type: Number })
  amount = 0;

  @property({ type: String, attribute: 'account-name' })
  accountName = '';

  @property({ type: String })
  currency = 'USD';

  @property({ type: String })
  locale?: string;

  /**
   * What the transaction already carries, which only a re-review by id ever
   * has: `GET /api/review/:id` answers with a full register row, and that row
   * may already have been categorized.
   */
  @property({ type: String, attribute: 'current-category' })
  currentCategory: string | null = null;

  @property({ type: String, attribute: 'current-vendor' })
  currentVendor: string | null = null;

  render() {
    const current = this.currentCategory ?? this.currentVendor;

    return html`
      <div class="headline">
        <h2 class="description">${this.description}</h2>
        <wc-money
          class="amount"
          .amount=${this.amount}
          currency=${this.currency}
          locale=${this.locale ?? nothing}
          variant="signed"
        ></wc-money>
      </div>
      <dl>
        <dt>Date</dt>
        <dd>${this.date}</dd>
        <dt>Account</dt>
        <dd>${this.accountName}</dd>
      </dl>
      ${current
        ? html`<p class="current">
            Currently ${this.currentCategory ?? 'uncategorized'}${this.currentVendor
              ? html` · ${this.currentVendor}`
              : nothing}
          </p>`
        : nothing}
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-review-card': WcReviewCard;
  }
}

import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import './wc-spinner.js';

/** One description a pattern would match, and how many carry it. */
export interface RuleTestMatchRow {
  description: string;
  count: number;
}

/**
 * What a rule pattern would match today, the dry run `nigel rules test` prints.
 *
 * Separate from the review form because the managers screen wants the same
 * panel beside its rule editor, and because it keeps the result type off the
 * form's prop surface.
 *
 * `aria-live` is the point of the component: the panel updates while the
 * cursor stays in the pattern box, and an update nobody is told about is an
 * update a screen-reader user never gets.
 */
@customElement('wc-rule-test-preview')
export class WcRuleTestPreview extends LitElement {
  static styles = css`
    :host {
      display: block;
      background: var(--wa-color-neutral-fill, rgb(0 0 0 / 4%));
      border-radius: var(--wa-radius-m, 8px);
      padding: var(--wa-space-s, 8px) var(--wa-space-m, 12px);
      font-family: var(--wa-font-family-sans);
      font-size: var(--wa-font-size-s, 13px);
      color: var(--wa-color-text);
    }

    .headline {
      display: flex;
      align-items: center;
      gap: var(--wa-space-xs, 6px);
      color: var(--wa-color-muted);
    }

    .error {
      color: var(--wa-color-danger, #b3261e);
    }

    ul {
      margin: var(--wa-space-xs, 6px) 0 0;
      padding-left: var(--wa-space-l, 16px);
      max-height: 10rem;
      overflow-y: auto;
    }

    li {
      overflow-wrap: anywhere;
    }

    .count {
      color: var(--wa-color-muted);
    }
  `;

  @property({ type: Boolean })
  busy = false;

  /** Null before anything has been tested. */
  @property({ attribute: false })
  result: { total: number; matches: RuleTestMatchRow[] } | null = null;

  /** A rejected pattern — an uncompilable regex is the usual one. */
  @property({ type: String })
  error: string | null = null;

  render() {
    return html`
      <div aria-live="polite">${this.renderBody()}</div>
    `;
  }

  private renderBody() {
    if (this.error) {
      return html`<p class="headline error">${this.error}</p>`;
    }

    if (this.busy) {
      return html`
        <p class="headline">
          <wc-spinner size="s" label="Testing the pattern"></wc-spinner>
          Checking what this matches…
        </p>
      `;
    }

    if (!this.result) {
      return html`<p class="headline">Type a pattern to see what it would match.</p>`;
    }

    if (this.result.total === 0) {
      return html`<p class="headline">Nothing matches this pattern yet.</p>`;
    }

    const noun = this.result.total === 1 ? 'transaction' : 'transactions';

    return html`
      <p class="headline">Matches ${this.result.total} ${noun}:</p>
      <ul>
        ${this.result.matches.map(
          (match) => html`
            <li>
              ${match.description}
              ${match.count > 1 ? html`<span class="count">×${match.count}</span>` : nothing}
            </li>
          `,
        )}
      </ul>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-rule-test-preview': WcRuleTestPreview;
  }
}

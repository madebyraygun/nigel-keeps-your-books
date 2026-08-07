import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import './wc-panel.js';
import './wc-notice-bar.js';
import './wc-money.js';

/**
 * The verdict on one reconciliation, in the two shapes it comes in.
 *
 * Mirrors `reconcile_manager.rs`'s result screen, with one addition: the
 * reconciled variant shows the statement figure alongside the calculated one,
 * where the TUI prints only the calculated. Seeing both is what makes a zero
 * discrepancy legible rather than merely asserted.
 *
 * The difference is emphasised with its own row, bold weight and an
 * `<abbr>`-free label rather than colour alone — the red is a second channel,
 * not the only one (WCAG 1.4.1), which is the same reason `wc-money` always
 * renders its sign.
 */
@customElement('wc-reconcile-result')
export class WcReconcileResult extends LitElement {
  static styles = css`
    :host {
      display: block;
    }

    wc-notice-bar {
      margin-bottom: var(--wa-space-m, 12px);
    }

    dl {
      display: grid;
      grid-template-columns: auto 1fr;
      gap: var(--wa-space-2xs, 4px) var(--wa-space-m, 12px);
      margin: 0;
      font-size: var(--wa-font-size-s, 13px);
    }

    dt {
      color: var(--wa-color-muted);
    }

    dd {
      margin: 0;
      justify-self: end;
      font-variant-numeric: tabular-nums;
    }

    dt.difference,
    dd.difference {
      font-weight: var(--wa-font-weight-semibold, 600);
      color: var(--wa-color-danger);
      padding-top: var(--wa-space-2xs, 4px);
      border-top: 1px solid var(--wa-color-border);
    }

    dd.difference wc-money {
      color: var(--wa-color-danger);
    }
  `;

  @property({ type: String })
  account = '';

  @property({ type: String })
  month = '';

  @property({ type: Boolean, attribute: 'is-reconciled' })
  isReconciled = false;

  @property({ type: Number, attribute: 'statement-balance' })
  statementBalance = 0;

  @property({ type: Number, attribute: 'calculated-balance' })
  calculatedBalance = 0;

  @property({ type: Number })
  discrepancy = 0;

  render() {
    return html`
      <wc-panel heading="Reconciliation result">
        <wc-notice-bar
          variant=${this.isReconciled ? 'success' : 'danger'}
          message=${this.isReconciled ? 'Reconciled!' : 'Discrepancy'}
        ></wc-notice-bar>

        <dl>
          <dt>Account</dt>
          <dd>${this.account}</dd>

          <dt>Month</dt>
          <dd>${this.month}</dd>

          <dt>Statement</dt>
          <dd>
            <wc-money .amount=${this.statementBalance} variant="plain"></wc-money>
          </dd>

          <dt>Calculated</dt>
          <dd>
            <wc-money .amount=${this.calculatedBalance} variant="plain"></wc-money>
          </dd>

          ${this.isReconciled
            ? nothing
            : html`
                <dt class="difference">Difference</dt>
                <dd class="difference">
                  <wc-money .amount=${this.discrepancy} variant="plain"></wc-money>
                </dd>
              `}
        </dl>
      </wc-panel>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-reconcile-result': WcReconcileResult;
  }
}

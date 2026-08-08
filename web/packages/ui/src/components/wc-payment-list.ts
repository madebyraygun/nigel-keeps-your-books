import { LitElement, html, css } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import './wc-money.js';

/** One recorded payment, oldest first as `payments` answers them. */
export interface PaymentRow {
  id: number | null;
  amount: number;
  paidDate: string;
  method: string;
  /** Present only for a payment `invoice sync` pulled from Stripe. */
  stripeCheckoutSessionId: string | null;
}

const METHOD_LABELS: Record<string, string> = {
  stripe: 'Stripe',
  ach: 'ACH',
  direct_deposit: 'Direct deposit',
  other: 'Other',
};

/** Humanize a known method, pass an unknown one through unchanged. */
export function paymentMethodLabel(method: string): string {
  return METHOD_LABELS[method] ?? method;
}

/**
 * An invoice's payment history.
 *
 * A Stripe payment is marked as one because it is the payment nobody typed:
 * it arrived through `invoice sync`, keyed by its checkout session, and that
 * is the difference worth showing when a figure is queried.
 */
@customElement('wc-payment-list')
export class WcPaymentList extends LitElement {
  static styles = css`
    :host {
      display: block;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    .scroll {
      overflow-x: auto;
    }

    table {
      width: 100%;
      border-collapse: collapse;
      font-size: var(--wa-font-size-s, 13px);
    }

    caption {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip-path: inset(50%);
      white-space: nowrap;
    }

    th,
    td {
      padding: var(--wa-space-xs, 6px) var(--wa-space-s, 8px);
      border-bottom: 1px solid var(--wa-color-border);
      text-align: start;
      white-space: nowrap;
    }

    th {
      color: var(--wa-color-muted);
      font-weight: var(--wa-font-weight-medium, 500);
    }

    td.end,
    th.end {
      text-align: end;
    }

    .synced {
      color: var(--wa-color-muted);
      font-size: var(--wa-font-size-s, 13px);
    }

    .empty {
      margin: 0;
      padding: var(--wa-space-s, 8px) 0;
      color: var(--wa-color-muted);
      font-size: var(--wa-font-size-s, 13px);
    }
  `;

  @property({ attribute: false })
  payments: PaymentRow[] = [];

  @property({ type: String })
  caption = 'Payments';

  @property({ type: String, attribute: 'empty-message' })
  emptyMessage = 'No payments recorded yet.';

  render() {
    if (this.payments.length === 0) {
      return html`<p class="empty" data-empty>${this.emptyMessage}</p>`;
    }

    return html`
      <div class="scroll">
        <table>
          <caption>
            ${this.caption}
          </caption>
          <thead>
            <tr>
              <th scope="col">Date</th>
              <th scope="col">Method</th>
              <th scope="col" class="end">Amount</th>
            </tr>
          </thead>
          <tbody>
            ${this.payments.map(
              (payment, index) => html`
                <tr data-row=${payment.id ?? index}>
                  <td>${payment.paidDate}</td>
                  <td>
                    ${paymentMethodLabel(payment.method)}
                    ${payment.stripeCheckoutSessionId
                      ? html`<span class="synced" data-synced>(synced)</span>`
                      : ''}
                  </td>
                  <td class="end">
                    <wc-money
                      .amount=${payment.amount}
                      variant="plain"
                      align="end"
                    ></wc-money>
                  </td>
                </tr>
              `,
            )}
          </tbody>
        </table>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-payment-list': WcPaymentList;
  }
}

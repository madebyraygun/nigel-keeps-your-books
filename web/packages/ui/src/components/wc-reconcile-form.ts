import { LitElement, html, css, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/select/select.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/button/button.js';

export interface ReconcileFormValue {
  /** Account name — the reconcile route addresses accounts by name. */
  account: string;
  /** `YYYY-MM`. */
  month: string;
  /** As typed: digits, a decimal point, a sign, and any commas. */
  balance: string;
}

export interface ReconcileFormErrors {
  account?: string;
  month?: string;
  balance?: string;
}

export interface NcReconcileChangeDetail {
  value: ReconcileFormValue;
}

export interface NcReconcileSubmitDetail {
  account: string;
  month: string;
  statementBalance: number;
}

export const EMPTY_RECONCILE_FORM: ReconcileFormValue = {
  account: '',
  month: '',
  balance: '',
};

/** What the TUI accepts in its balance field, character for character. */
const BALANCE_CHARS = /^[-0-9.,]*$/;

const MONTH_PATTERN = /^\d{4}-(0[1-9]|1[0-2])$/;

/**
 * A typed statement balance as a number, or null if it is not one.
 *
 * Commas come out first, matching `reconcile_manager.rs`, which parses
 * `self.balance.replace(',', "")` — a balance copied off a statement arrives
 * with thousands separators far more often than not.
 */
export function parseStatementBalance(raw: string): number | null {
  const cleaned = raw.replace(/,/g, '').trim();
  if (cleaned === '' || !BALANCE_CHARS.test(cleaned)) return null;
  const parsed = Number(cleaned);
  return Number.isFinite(parsed) ? parsed : null;
}

/**
 * Tidy a parsed balance back into the field: two decimals, grouped.
 *
 * Decimal style rather than currency, because the field renders its own `$`
 * prefix. `en-US` rather than the reader's locale, because the output has to
 * survive `parseStatementBalance`, which reads a comma as a separator and a dot
 * as the decimal point: on a comma-decimal locale a tidied `500.25` would come
 * back as `500,25` and re-parse as 50025.
 */
export function formatStatementBalance(amount: number): string {
  return new Intl.NumberFormat('en-US', {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(amount);
}

/**
 * Validation, in the TUI's words.
 *
 * The three messages are `reconcile_manager.rs`'s verbatim; a user who moves
 * between the two surfaces should not have to learn the same rule twice.
 */
export function validateReconcileForm(value: ReconcileFormValue): ReconcileFormErrors {
  const errors: ReconcileFormErrors = {};

  if (value.account.trim() === '') errors.account = 'Account is required';

  if (value.month.trim() === '') errors.month = 'Month is required (YYYY-MM)';
  else if (!MONTH_PATTERN.test(value.month.trim())) errors.month = 'Month must be YYYY-MM';

  if (value.balance.trim() === '') errors.balance = 'Balance is required';
  else if (parseStatementBalance(value.balance) === null) {
    errors.balance = 'Invalid balance amount';
  }

  return errors;
}

/**
 * Account, month and statement balance — the three things reconciling asks for.
 *
 * One component rather than three loose `wa-*` on the screen, following
 * `wc-import-form`: the fields are submitted together or not at all, and the
 * component-first rule keeps primitives out of screens. Controlled, so the
 * screen owns the values and this only reports what they would become.
 *
 * The balance field is the app's only currency input. It carries a `$` prefix
 * rather than expecting one to be typed, tidies itself on blur, and accepts
 * the same characters the TUI does — including the commas a figure copied off
 * a paper statement arrives with.
 */
@customElement('wc-reconcile-form')
export class WcReconcileForm extends LitElement {
  static styles = css`
    :host {
      display: block;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    form {
      display: grid;
      gap: var(--wa-space-m, 12px);
    }

    .row {
      display: flex;
      flex-wrap: wrap;
      gap: var(--wa-space-m, 12px);
    }

    .row > * {
      flex: 1 1 12rem;
      min-width: 0;
    }

    /* Money reads as money: same face and same digit widths as wc-money. */
    wa-input.balance::part(input) {
      font-family: var(--nc-font-money, ui-monospace, monospace);
      font-variant-numeric: tabular-nums;
      text-align: end;
    }

    .prefix {
      color: var(--wa-color-muted);
      padding-inline-start: var(--wa-space-s, 8px);
    }

    .error {
      margin: var(--wa-space-2xs, 4px) 0 0;
      font-size: var(--wa-font-size-s, 13px);
      color: var(--wa-color-danger);
    }

    .hint {
      margin: var(--wa-space-2xs, 4px) 0 0;
      font-size: var(--wa-font-size-s, 13px);
      color: var(--wa-color-muted);
    }

    .actions {
      display: flex;
      gap: var(--wa-space-s, 8px);
    }
  `;

  @property({ attribute: false })
  accounts: string[] = [];

  @property({ attribute: false })
  value: ReconcileFormValue = EMPTY_RECONCILE_FORM;

  /** Errors the server reported — a 409 on the month, a 404 on the account. */
  @property({ attribute: false })
  errors: ReconcileFormErrors = {};

  @property({ type: Boolean, reflect: true })
  busy = false;

  /** What this form found itself, cleared on every edit and every attempt. */
  @state()
  private localErrors: ReconcileFormErrors = {};

  private get disabled(): boolean {
    return this.busy || this.accounts.length === 0;
  }

  private errorFor(field: keyof ReconcileFormErrors): string | undefined {
    return this.localErrors[field] ?? this.errors[field];
  }

  private static valueOf(event: Event): string {
    return (event.target as HTMLElement & { value: string }).value ?? '';
  }

  private emit(patch: Partial<ReconcileFormValue>): void {
    // An edit invalidates what the last attempt concluded, on either side.
    this.localErrors = {};
    this.dispatchEvent(
      new CustomEvent<NcReconcileChangeDetail>('nc-reconcile-change', {
        detail: { value: { ...this.value, ...patch } },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private handleAccount = (event: Event): void => {
    this.emit({ account: WcReconcileForm.valueOf(event) });
  };

  private handleMonth = (event: Event): void => {
    this.emit({ month: WcReconcileForm.valueOf(event) });
  };

  private handleBalance = (event: Event): void => {
    this.emit({ balance: WcReconcileForm.valueOf(event) });
  };

  /** Tidy on the way out, and only when there is something valid to tidy. */
  private handleBalanceBlur = (): void => {
    const parsed = parseStatementBalance(this.value.balance);
    if (parsed === null) return;
    const formatted = formatStatementBalance(parsed);
    if (formatted !== this.value.balance) this.emit({ balance: formatted });
  };

  private handleSubmit = (event: Event): void => {
    event.preventDefault();
    if (this.disabled) return;

    const errors = validateReconcileForm(this.value);
    this.localErrors = errors;
    if (Object.keys(errors).length > 0) return;

    // Non-null: validation just established that it parses.
    const statementBalance = parseStatementBalance(this.value.balance) as number;
    this.dispatchEvent(
      new CustomEvent<NcReconcileSubmitDetail>('nc-reconcile-submit', {
        detail: {
          account: this.value.account,
          month: this.value.month.trim(),
          statementBalance,
        },
        bubbles: true,
        composed: true,
      }),
    );
  };

  render() {
    return html`
      <form @submit=${this.handleSubmit} novalidate>
        <div class="row">${this.renderAccount()} ${this.renderMonth()}</div>
        ${this.renderBalance()}
        <div class="actions">
          <wa-button
            type="submit"
            variant="brand"
            ?disabled=${this.disabled}
            ?loading=${this.busy}
          >
            Reconcile
          </wa-button>
        </div>
      </form>
    `;
  }

  private renderAccount() {
    const error = this.errorFor('account');

    return html`
      <div>
        <wa-select
          class="account"
          label="Account"
          size="s"
          value=${this.value.account}
          ?disabled=${this.disabled}
          @change=${this.handleAccount}
        >
          ${this.accounts.map(
            (name) => html`<wa-option value=${name}>${name}</wa-option>`,
          )}
        </wa-select>
        ${this.accounts.length === 0
          ? html`<p class="hint">No accounts yet — add one before reconciling.</p>`
          : nothing}
        ${error ? html`<p class="error" role="alert">${error}</p>` : nothing}
      </div>
    `;
  }

  private renderMonth() {
    const error = this.errorFor('month');

    return html`
      <div>
        <wa-input
          class="month"
          type="month"
          label="Month"
          size="s"
          placeholder="YYYY-MM"
          value=${this.value.month}
          ?disabled=${this.disabled}
          @input=${this.handleMonth}
        ></wa-input>
        ${error ? html`<p class="error" role="alert">${error}</p>` : nothing}
      </div>
    `;
  }

  /**
   * `inputmode` rather than `type="number"`: a number field rejects the commas
   * a statement figure is copied with, and silently empties itself instead of
   * saying so.
   */
  private renderBalance() {
    const error = this.errorFor('balance');

    return html`
      <div>
        <wa-input
          class="balance"
          label="Statement balance"
          size="s"
          inputmode="decimal"
          autocomplete="off"
          value=${this.value.balance}
          ?disabled=${this.disabled}
          @input=${this.handleBalance}
          @blur=${this.handleBalanceBlur}
        >
          <span slot="start" class="prefix" aria-hidden="true">$</span>
        </wa-input>
        ${error ? html`<p class="error" role="alert">${error}</p>` : nothing}
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-reconcile-form': WcReconcileForm;
  }

  interface HTMLElementEventMap {
    'nc-reconcile-change': CustomEvent<NcReconcileChangeDetail>;
    'nc-reconcile-submit': CustomEvent<NcReconcileSubmitDetail>;
  }
}

import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/select/select.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import { ACCOUNT_TYPES, accountTypeLabel } from './account-type.js';

export type WcAccountFormMode = 'create' | 'rename';

export interface AccountFormValue {
  name: string;
  accountType: string;
  institution: string;
  lastFour: string;
}

export interface AccountFormErrors {
  name?: string;
  lastFour?: string;
}

export interface NcAccountFormChangeDetail {
  value: AccountFormValue;
}

export const EMPTY_ACCOUNT_FORM: AccountFormValue = {
  name: '',
  accountType: ACCOUNT_TYPES[0],
  institution: '',
  lastFour: '',
};

/**
 * What the form can reject before the server sees it.
 *
 * The last-four rule is `account_manager.rs`'s, wording included. It lives in
 * the TUI and nowhere else — `add_account` and the route both accept any string
 * — so without it the web would be the laxer of the two surfaces.
 */
export function validateAccountForm(value: AccountFormValue): AccountFormErrors {
  const errors: AccountFormErrors = {};

  if (value.name.trim() === '') errors.name = 'Name is required';

  const lastFour = value.lastFour.trim();
  if (lastFour !== '' && !/^\d{4}$/.test(lastFour)) {
    errors.lastFour = 'Last four must be exactly 4 digits';
  }

  return errors;
}

/**
 * The account add/rename field group.
 *
 * Controlled: it renders `value` and emits every edit, because the screen owns
 * the request and has to know what it would send. Rename mode shows one field,
 * which is all `PATCH /api/accounts/:id` accepts — type, institution and last
 * four are creation-time facts, and the other three render as text so the form
 * does not silently imply otherwise.
 */
@customElement('wc-account-form')
export class WcAccountForm extends LitElement {
  static styles = css`
    :host {
      display: block;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    .fields {
      display: grid;
      gap: var(--wa-space-m, 12px);
      min-width: 20rem;
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

    .fixed {
      margin: 0;
      display: grid;
      grid-template-columns: max-content 1fr;
      gap: var(--wa-space-2xs, 4px) var(--wa-space-m, 12px);
      font-size: var(--wa-font-size-s, 13px);
    }

    .fixed dt {
      color: var(--wa-color-muted);
    }

    .fixed dd {
      margin: 0;
    }

    .hint {
      margin: 0;
      color: var(--wa-color-muted);
      font-size: var(--wa-font-size-s, 13px);
    }
  `;

  @property({ type: String, reflect: true })
  mode: WcAccountFormMode = 'create';

  @property({ attribute: false })
  value: AccountFormValue = EMPTY_ACCOUNT_FORM;

  @property({ attribute: false })
  errors: AccountFormErrors = {};

  @property({ type: Boolean })
  disabled = false;

  private emit(next: Partial<AccountFormValue>): void {
    this.dispatchEvent(
      new CustomEvent<NcAccountFormChangeDetail>('nc-account-form-change', {
        detail: { value: { ...this.value, ...next } },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private handleField(field: keyof AccountFormValue) {
    return (event: Event) => {
      const input = event.target as HTMLInputElement;
      this.emit({ [field]: input.value } as Partial<AccountFormValue>);
    };
  }

  render() {
    return html`
      <div class="fields">
        <div>
          <wa-input
            data-name
            label="Name"
            autocomplete="off"
            value=${this.value.name}
            ?disabled=${this.disabled}
            @input=${this.handleField('name')}
          ></wa-input>
          ${this.errors.name
            ? html`<p class="error" role="alert">${this.errors.name}</p>`
            : nothing}
        </div>
        ${this.mode === 'create' ? this.renderCreateFields() : this.renderFixed()}
      </div>
    `;
  }

  private renderCreateFields() {
    return html`
      <wa-select
        data-type
        label="Type"
        value=${this.value.accountType}
        ?disabled=${this.disabled}
        @change=${this.handleField('accountType')}
      >
        ${ACCOUNT_TYPES.map(
          (type) =>
            html`<wa-option value=${type}>${accountTypeLabel(type)}</wa-option>`,
        )}
      </wa-select>
      <div class="row">
        <wa-input
          data-institution
          label="Institution"
          autocomplete="off"
          value=${this.value.institution}
          ?disabled=${this.disabled}
          @input=${this.handleField('institution')}
        ></wa-input>
        <div>
          <wa-input
            data-last-four
            label="Last four"
            inputmode="numeric"
            maxlength="4"
            autocomplete="off"
            value=${this.value.lastFour}
            ?disabled=${this.disabled}
            @input=${this.handleField('lastFour')}
          ></wa-input>
          ${this.errors.lastFour
            ? html`<p class="error" role="alert">${this.errors.lastFour}</p>`
            : nothing}
        </div>
      </div>
    `;
  }

  private renderFixed() {
    return html`
      <dl class="fixed">
        <dt>Type</dt>
        <dd>${accountTypeLabel(this.value.accountType)}</dd>
        <dt>Institution</dt>
        <dd>${this.value.institution || '—'}</dd>
        <dt>Last four</dt>
        <dd>${this.value.lastFour || '—'}</dd>
      </dl>
      <p class="hint">
        Type, institution and last four are set when the account is created.
      </p>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-account-form': WcAccountForm;
  }
  interface HTMLElementEventMap {
    'nc-account-form-change': CustomEvent<NcAccountFormChangeDetail>;
  }
}

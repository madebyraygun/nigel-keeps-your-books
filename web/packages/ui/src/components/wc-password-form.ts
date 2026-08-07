import { LitElement, html, css, nothing } from 'lit';
import { customElement, property, state, query } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/button/button.js';

export type WcPasswordMode = 'set' | 'change' | 'remove';

/** What the form hands back. Only the fields its mode actually collects. */
export interface NcPasswordSubmitDetail {
  mode: WcPasswordMode;
  currentPassword?: string;
  newPassword?: string;
}

const SUBMIT_LABELS: Record<WcPasswordMode, string> = {
  set: 'Encrypt database',
  change: 'Change password',
  remove: 'Remove password',
};

/**
 * The set / change / remove password field group.
 *
 * "New password plus confirmation, with a mismatch message" is visual behavior,
 * so it lives here rather than being retyped into every screen that needs it.
 * The confirmation value never leaves the component — it exists to catch a typo,
 * and the server has no use for it.
 */
@customElement('wc-password-form')
export class WcPasswordForm extends LitElement {
  static styles = css`
    :host {
      display: block;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    form {
      display: grid;
      gap: var(--wa-space-m, 12px);
      max-width: 24rem;
    }

    .message {
      margin: 0;
      font-size: var(--wa-font-size-s, 13px);
      min-height: 1.25rem;
    }

    .error {
      color: var(--wa-color-danger);
    }

    .actions {
      display: flex;
      gap: var(--wa-space-s, 8px);
    }
  `;

  @property({ type: String, reflect: true })
  mode: WcPasswordMode = 'set';

  @property({ type: Boolean, reflect: true })
  busy = false;

  /** A failure from the server, shown alongside anything found locally. */
  @property({ type: String })
  error = '';

  /** Validation the form found itself, cleared on every new attempt. */
  @state()
  private localError = '';

  @query('[data-current]')
  private currentInput?: HTMLElement & { value: string };

  @query('[data-new]')
  private newInput?: HTMLElement & { value: string };

  @query('[data-confirm]')
  private confirmInput?: HTMLElement & { value: string };

  private get needsCurrent(): boolean {
    return this.mode !== 'set';
  }

  private get needsNew(): boolean {
    return this.mode !== 'remove';
  }

  private clearFields(): void {
    for (const field of [this.currentInput, this.newInput, this.confirmInput]) {
      if (field) field.value = '';
    }
  }

  private handleSubmit = (event: Event) => {
    event.preventDefault();
    if (this.busy) return;

    this.localError = '';
    const currentPassword = this.currentInput?.value ?? '';
    const newPassword = this.newInput?.value ?? '';
    const confirmation = this.confirmInput?.value ?? '';

    if (this.needsCurrent && currentPassword.length === 0) {
      this.localError = 'Enter the current password.';
      return;
    }
    if (this.needsNew) {
      if (newPassword.trim().length === 0) {
        // The wording the terminal prompt uses, so the two agree.
        this.localError = 'Password cannot be empty.';
        return;
      }
      if (newPassword !== confirmation) {
        this.localError = 'Passwords do not match.';
        return;
      }
    }

    const detail: NcPasswordSubmitDetail = { mode: this.mode };
    if (this.needsCurrent) detail.currentPassword = currentPassword;
    if (this.needsNew) detail.newPassword = newPassword;

    this.clearFields();
    this.dispatchEvent(
      new CustomEvent<NcPasswordSubmitDetail>('nc-password-submit', {
        detail,
        bubbles: true,
        composed: true,
      }),
    );
  };

  render() {
    const message = this.localError || this.error;
    return html`
      <form @submit=${this.handleSubmit}>
        ${this.needsCurrent
          ? html`<wa-input
              data-current
              type="password"
              label="Current password"
              autocomplete="current-password"
              ?disabled=${this.busy}
            ></wa-input>`
          : nothing}
        ${this.needsNew
          ? html`
              <wa-input
                data-new
                type="password"
                label="New password"
                autocomplete="new-password"
                ?disabled=${this.busy}
              ></wa-input>
              <wa-input
                data-confirm
                type="password"
                label="Confirm new password"
                autocomplete="new-password"
                ?disabled=${this.busy}
              ></wa-input>
            `
          : nothing}
        <p class="message ${message ? 'error' : ''}" role="status" aria-live="polite">
          ${message}
        </p>
        <div class="actions">
          <wa-button
            type="submit"
            variant=${this.mode === 'remove' ? 'danger' : 'brand'}
            ?disabled=${this.busy}
          >
            ${this.busy ? 'Working…' : SUBMIT_LABELS[this.mode]}
          </wa-button>
        </div>
      </form>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-password-form': WcPasswordForm;
  }
}

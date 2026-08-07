import { LitElement, html, css, nothing } from 'lit';
import { customElement, property, query } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/button/button.js';
import '../icons/icons.js';

/** What the card hands back when the form is submitted. */
export interface NcUnlockDetail {
  password: string;
}

/**
 * The gate in front of an encrypted database — the web counterpart of the
 * splash screen's password prompt.
 *
 * The password lives in the input element and in the one event this dispatches,
 * and nowhere else: it is never reflected to an attribute, never written to a
 * property that survives submit, and never stored.
 */
@customElement('wc-unlock-card')
export class WcUnlockCard extends LitElement {
  static styles = css`
    :host {
      display: block;
      max-width: 26rem;
      width: 100%;
      padding: var(--wa-space-xl, 24px);
      background: var(--wa-color-surface);
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-l, 12px);
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    .brand {
      display: flex;
      align-items: center;
      gap: var(--wa-space-s, 8px);
      margin-bottom: var(--wa-space-l, 16px);
    }

    .lock {
      --nc-icon-size: 24px;
      color: var(--wa-color-brand);
    }

    .heading {
      margin: 0;
      font-size: var(--wa-font-size-lg, 16px);
      font-weight: var(--wa-font-weight-semibold, 600);
    }

    .hint {
      margin: 0 0 var(--wa-space-m, 12px);
      color: var(--wa-color-muted);
      font-size: var(--wa-font-size-s, 13px);
    }

    form {
      display: grid;
      gap: var(--wa-space-m, 12px);
    }

    .status {
      min-height: 1.25rem;
      font-size: var(--wa-font-size-s, 13px);
    }

    .error {
      color: var(--wa-color-danger);
      margin: 0;
    }

    .attempts,
    .countdown {
      color: var(--wa-color-muted);
      margin: var(--wa-space-2xs, 4px) 0 0;
    }
  `;

  /** Usually the business name; falls back to the product name. */
  @property({ type: String })
  heading = 'Nigel';

  @property({ type: String })
  error = '';

  /** `null` when the server has not said, so nothing is claimed. */
  @property({ type: Number, attribute: 'attempts-remaining' })
  attemptsRemaining: number | null = null;

  /** A request is in flight; the form is disabled until it answers. */
  @property({ type: Boolean, reflect: true })
  busy = false;

  /**
   * Seconds the server is expected to hold the in-flight answer back.
   *
   * The throttle is served *by the server*, before it replies — so this counts
   * down during the request rather than locking the form afterwards, which
   * would charge the same penalty twice.
   */
  @property({ type: Number, attribute: 'countdown-seconds' })
  countdownSeconds = 0;

  @query('wa-input')
  private input?: HTMLElement & { value: string; focus(): void };

  /** Put the cursor where the user is about to type. */
  focusInput(): void {
    void this.updateComplete.then(() => this.input?.focus());
  }

  private handleSubmit = (event: Event) => {
    event.preventDefault();
    if (this.busy) return;

    const password = this.input?.value ?? '';
    if (password.length === 0) return;

    // Cleared immediately: a failed attempt should not leave the password
    // sitting in the DOM waiting for the next screenshot.
    if (this.input) this.input.value = '';

    this.dispatchEvent(
      new CustomEvent<NcUnlockDetail>('nc-unlock', {
        detail: { password },
        bubbles: true,
        composed: true,
      }),
    );
  };

  private renderStatus() {
    if (this.busy && this.countdownSeconds > 0) {
      return html`<p class="countdown">
        Too many attempts — checking in ${this.countdownSeconds}s.
      </p>`;
    }
    if (!this.error) return nothing;
    return html`
      <p class="error">${this.error}</p>
      ${this.attemptsRemaining !== null
        ? html`<p class="attempts">
            ${this.attemptsRemaining === 0
              ? 'No attempts left before the wait gets longer.'
              : `${this.attemptsRemaining} ${
                  this.attemptsRemaining === 1 ? 'attempt' : 'attempts'
                } remaining.`}
          </p>`
        : nothing}
    `;
  }

  render() {
    return html`
      <div class="brand">
        <wc-icon-lock class="lock"></wc-icon-lock>
        <h1 class="heading">${this.heading}</h1>
      </div>
      <p class="hint">This database is encrypted. Enter its password to continue.</p>
      <form @submit=${this.handleSubmit}>
        <wa-input
          type="password"
          label="Database password"
          autocomplete="current-password"
          ?disabled=${this.busy}
          password-toggle
        ></wa-input>
        <div class="status" role="status" aria-live="polite">
          ${this.renderStatus()}
        </div>
        <wa-button type="submit" variant="brand" ?disabled=${this.busy}>
          ${this.busy ? 'Unlocking…' : 'Unlock'}
        </wa-button>
      </form>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-unlock-card': WcUnlockCard;
  }
}

import { LitElement, html, css, type TemplateResult } from 'lit';
import { customElement, state } from 'lit/decorators.js';
import '@nigel/ui';
import type { NcUnlockDetail } from '@nigel/ui';

import { SignalWatcher } from '../mixins/signal-watcher.js';
import { getAppStore, type AppStore } from '../state/app-store.js';
import type { ScreenContext } from './context.js';

/** The documented ceiling on the server's unlock backoff. */
const MAX_BACKOFF_MS = 30_000;

/**
 * The unlock gate for an encrypted database.
 *
 * Rendered instead of the app shell, not inside it: while the database is
 * locked there is no sidebar and no screen, so nothing exists that could fetch
 * data before the password arrives.
 */
@customElement('nigel-unlock-screen')
export class NigelUnlockScreen extends SignalWatcher(LitElement) {
  static styles = css`
    :host {
      display: grid;
      place-items: center;
      min-height: 100vh;
      padding: var(--wa-space-l, 16px);
      background: var(--wa-color-bg);
    }
  `;

  @state() private error = '';
  @state() private attemptsRemaining: number | null = null;
  @state() private busy = false;
  @state() private countdownSeconds = 0;

  /**
   * The delay the server held the last failed answer back by.
   *
   * The throttle is served *before* the response, so it is felt during the next
   * request rather than after this one. Locking the form for the same duration
   * on top would charge the penalty twice.
   */
  private lastRetryAfterMs = 0;
  private ticker: ReturnType<typeof setInterval> | null = null;

  private store: AppStore = getAppStore();

  disconnectedCallback(): void {
    this.stopCountdown();
    super.disconnectedCallback();
  }

  private startCountdown(): void {
    // The ladder doubles each time, so the next hold is twice the last one.
    const expected = Math.min(this.lastRetryAfterMs * 2, MAX_BACKOFF_MS);
    if (expected <= 0) return;

    this.countdownSeconds = Math.ceil(expected / 1000);
    this.ticker = setInterval(() => {
      this.countdownSeconds = Math.max(0, this.countdownSeconds - 1);
      if (this.countdownSeconds === 0) this.stopCountdown();
    }, 1000);
  }

  private stopCountdown(): void {
    if (this.ticker !== null) {
      clearInterval(this.ticker);
      this.ticker = null;
    }
    this.countdownSeconds = 0;
  }

  private handleUnlock = async (event: CustomEvent<NcUnlockDetail>) => {
    this.busy = true;
    this.error = '';
    this.startCountdown();

    const outcome = await this.store.unlock(event.detail.password);

    this.stopCountdown();
    this.busy = false;

    if (outcome.ok) {
      this.attemptsRemaining = null;
      this.lastRetryAfterMs = 0;
      return;
    }

    this.error = outcome.message;
    this.attemptsRemaining = outcome.attemptsRemaining ?? null;
    this.lastRetryAfterMs = outcome.retryAfterMs ?? 0;
  };

  render() {
    return html`
      <wc-unlock-card
        heading=${this.store.companyName.get()}
        error=${this.error}
        .attemptsRemaining=${this.attemptsRemaining}
        ?busy=${this.busy}
        .countdownSeconds=${this.countdownSeconds}
        @nc-unlock=${this.handleUnlock}
      ></wc-unlock-card>
    `;
  }
}

export function renderUnlock(_ctx: ScreenContext): TemplateResult {
  return html`<nigel-unlock-screen></nigel-unlock-screen>`;
}

declare global {
  interface HTMLElementTagNameMap {
    'nigel-unlock-screen': NigelUnlockScreen;
  }
}

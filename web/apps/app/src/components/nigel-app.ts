import { LitElement, html, css, nothing } from 'lit';
import { customElement, state } from 'lit/decorators.js';
import '@nigel/ui';
import { dispatchNcToast } from '@nigel/ui';

import { SignalWatcher } from '../mixins/signal-watcher.js';
import { FetchApiClient, appUnauthorized, type ApiClient } from '../api/index.js';
import {
  getAppStore,
  initializeAppStore,
  type AppStore,
} from '../state/app-store.js';
import { parseHash, screenToHash, type Route } from '../screens/hash-route.js';
import { DEFAULT_SCREEN, navItems, screenDef } from '../screens/registry.js';

/**
 * Root container: owns the api client, the store, and the route.
 *
 * Routing is one-directional. Navigation writes `location.hash` and nothing
 * else; the hashchange listener is the only thing that updates `route`. That
 * is what makes the back button and a pasted deep link behave the same as a
 * click.
 */
@customElement('nigel-app')
export class NigelApp extends SignalWatcher(LitElement) {
  static styles = css`
    :host {
      display: block;
      height: 100vh;
    }

    .boot {
      display: grid;
      place-items: center;
      height: 100vh;
      background: var(--wa-color-bg);
    }

    .banner {
      color: var(--wa-color-text);
    }

    .banner strong {
      color: var(--wa-color-danger);
    }

    .retry {
      margin-left: var(--wa-space-s, 8px);
    }
  `;

  /** Overridable so tests can drive the app with a fake transport. */
  client: ApiClient = new FetchApiClient();

  @state()
  private route: Route = { screen: DEFAULT_SCREEN, params: new URLSearchParams() };

  private store: AppStore | null = null;
  private reportedError: string | null = null;

  connectedCallback(): void {
    super.connectedCallback();
    this.store = initializeAppStore(this.client);
    window.addEventListener('hashchange', this.handleHashChange);
    this.syncRouteFromHash();
    void this.store.refreshStatus();
  }

  disconnectedCallback(): void {
    window.removeEventListener('hashchange', this.handleHashChange);
    super.disconnectedCallback();
  }

  private handleHashChange = (): void => {
    this.syncRouteFromHash();
  };

  private syncRouteFromHash(): void {
    if (!window.location.hash) {
      // Seed the address bar so a reload lands on the same screen.
      window.location.hash = screenToHash(DEFAULT_SCREEN);
      return;
    }
    this.route = parseHash(window.location.hash);
  }

  private handleNavigate = (event: CustomEvent<{ id: string }>): void => {
    window.location.hash = `#/${event.detail.id}`;
  };

  private handleRetry = (): void => {
    this.reportedError = null;
    void this.store?.refreshStatus();
  };

  /** Surface a status failure once per occurrence rather than every render. */
  private announceError(message: string): void {
    if (this.reportedError === message) return;
    this.reportedError = message;
    dispatchNcToast(this, { message, variant: 'danger', duration: 0 });
  }

  render() {
    const store = this.store ?? getAppStore();
    const status = store.status.get();
    const error = store.statusError.get();

    if (error) this.announceError(error.message);

    if (!status && !error) {
      return html`
        <div class="boot">
          <wc-spinner size="l" show-label label="Connecting to nigel"></wc-spinner>
        </div>
      `;
    }

    const locked = store.locked.get();
    // While locked the unlock screen is the only reachable one, but the hash
    // is left alone so unlocking returns the user to where they were headed.
    const screen = screenDef(locked ? 'unlock' : this.route.screen);

    document.title = `${screen.title} · ${store.companyName.get()}`;

    return html`
      <wc-app-shell screen-title=${screen.title}>
        <wc-nav-sidebar
          slot="sidebar"
          .items=${navItems({ disabled: locked })}
          active=${screen.id}
          app-name=${store.companyName.get()}
          @nc-navigate=${this.handleNavigate}
        ></wc-nav-sidebar>
        ${this.renderBanner(error?.message ?? null)} ${screen.render()}
      </wc-app-shell>
    `;
  }

  private renderBanner(errorMessage: string | null) {
    if (appUnauthorized.get()) {
      return html`
        <div slot="banner" class="banner">
          <strong>Session expired.</strong> Reopen the link
          <code>nigel serve</code> printed — the browser cannot mint a new
          session token on its own.
        </div>
      `;
    }
    if (errorMessage) {
      return html`
        <div slot="banner" class="banner">
          <strong>Could not load status.</strong> ${errorMessage}
          <button class="retry" type="button" @click=${this.handleRetry}>
            Retry
          </button>
        </div>
      `;
    }
    return nothing;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'nigel-app': NigelApp;
  }
}

import { LitElement, html, css, nothing, type TemplateResult } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/button/button.js';
import '@nigel/ui';
import { dispatchNcToast } from '@nigel/ui';

import { SignalWatcher } from '../mixins/signal-watcher.js';
import { ApiError, type ApiClient } from '../api/index.js';
import { getAppStore, type AppStore } from '../state/app-store.js';
import {
  initializeDashboardStore,
  type DashboardStore,
} from '../state/dashboard-store.js';
import { cashflowBuckets } from './dashboard-data.js';
import type { ScreenContext } from './context.js';

/**
 * The web counterpart of the TUI's home screen: year-to-date profit and loss,
 * account balances, twelve months of cash flow, and what needs review.
 *
 * The four figures are fetched in parallel and rendered independently, so a
 * failure shows up on the card it belongs to rather than emptying the screen.
 */
@customElement('nigel-dashboard-screen')
export class NigelDashboardScreen extends SignalWatcher(LitElement) {
  static styles = css`
    :host {
      display: grid;
      gap: var(--wa-space-l, 16px);
      align-content: start;
      padding: var(--wa-space-l, 16px);
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    .toolbar {
      display: flex;
      align-items: center;
      gap: var(--wa-space-s, 8px);
      flex-wrap: wrap;
    }

    .spacer {
      flex: 1 1 auto;
    }

    .flagged {
      display: inline-flex;
      align-items: center;
      gap: var(--wa-space-xs, 6px);
      padding: var(--wa-space-2xs, 4px) var(--wa-space-s, 8px);
      border: 1px solid var(--nc-color-flagged);
      border-radius: var(--wa-radius-pill, 999px);
      color: var(--wa-color-text);
      background: var(--wa-color-surface);
      font-size: var(--wa-font-size-s, 13px);
      text-decoration: none;
    }

    .flagged:hover {
      background: var(--wa-color-surface-alt);
    }

    .flagged:focus-visible {
      outline: 2px solid var(--wa-color-focus);
      outline-offset: 2px;
    }

    .count {
      font-weight: var(--wa-font-weight-bold, 700);
      color: var(--nc-color-flagged);
    }

    .stats {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(14rem, 1fr));
      gap: var(--wa-space-m, 12px);
    }

    .panels {
      display: grid;
      grid-template-columns: minmax(18rem, 1fr) minmax(0, 2fr);
      gap: var(--wa-space-m, 12px);
      align-items: start;
    }

    @media (max-width: 60rem) {
      .panels {
        grid-template-columns: 1fr;
      }
    }
  `;

  /** Supplied by the registry from the screen context. */
  @property({ attribute: false })
  client!: ApiClient;

  /** Dismissal lasts for the session — the server keeps no record of it. */
  @state() private updateDismissed = false;

  private appStore: AppStore = getAppStore();
  private store: DashboardStore | null = null;
  private reportedErrors = new Set<string>();

  connectedCallback(): void {
    super.connectedCallback();
    this.store = initializeDashboardStore(this.client);
    void this.store.load();
  }

  private handleRefresh = () => {
    // A refresh is the moment to re-report anything still broken.
    this.reportedErrors.clear();
    void this.store?.load();
    void this.appStore.refreshStatus();
  };

  /**
   * Toast a failure once.
   *
   * Render runs on every signal change, and a persistent error would otherwise
   * raise a fresh toast each time — the inline message on the card is the
   * durable report, the toast is only the announcement.
   */
  private announce(error: ApiError | null): void {
    if (!error || this.reportedErrors.has(error.message)) return;
    this.reportedErrors.add(error.message);
    dispatchNcToast(this, { message: error.message, variant: 'danger' });
  }

  private renderUpdateNotice() {
    const version = this.appStore.status.get()?.updateAvailable;
    if (!version || this.updateDismissed) return nothing;

    return html`
      <wc-notice-bar
        variant="warning"
        icon="wc-icon-download"
        message="Nigel v${version} is available. Run nigel update to install it."
        dismissible
        @nc-notice-dismiss=${() => (this.updateDismissed = true)}
      ></wc-notice-bar>
    `;
  }

  private renderToolbar(store: DashboardStore) {
    const flagged = store.flaggedCount.get();

    return html`
      <div class="toolbar">
        ${flagged > 0
          ? html`
              <a class="flagged" href="#/review">
                <wc-icon-flag></wc-icon-flag>
                <span class="count">${flagged}</span>
                ${flagged === 1 ? 'transaction needs' : 'transactions need'}
                review
              </a>
            `
          : nothing}
        <span class="spacer"></span>
        <wa-button
          appearance="plain"
          size="s"
          ?disabled=${store.busy.get()}
          @click=${this.handleRefresh}
        >
          <wc-icon-refresh label="Refresh dashboard"></wc-icon-refresh>
        </wa-button>
      </div>
    `;
  }

  private renderStats(store: DashboardStore) {
    const pnl = store.pnl.data.get();
    const loading = store.pnl.loading.get();
    const error = store.pnl.error.get()?.message ?? '';
    const year = new Date().getFullYear();

    return html`
      <div class="stats">
        <wc-stat-card
          label="YTD Income"
          .amount=${pnl?.totalIncome ?? 0}
          hint=${String(year)}
          ?loading=${loading}
          error=${error}
          @nc-retry=${() => void store.reloadPnl()}
        ></wc-stat-card>
        <wc-stat-card
          label="YTD Expenses"
          .amount=${pnl?.totalExpenses ?? 0}
          hint=${String(year)}
          ?loading=${loading}
          error=${error}
          @nc-retry=${() => void store.reloadPnl()}
        ></wc-stat-card>
        <wc-stat-card
          label="Net"
          .amount=${pnl?.net ?? 0}
          hint="Cash basis"
          ?loading=${loading}
          error=${error}
          @nc-retry=${() => void store.reloadPnl()}
        ></wc-stat-card>
      </div>
    `;
  }

  private renderPanels(store: DashboardStore) {
    const balance = store.balance.data.get();
    const cashflow = store.cashflow.data.get();
    const chart = cashflow
      ? cashflowBuckets(cashflow)
      : { buckets: [], caption: '' };

    return html`
      <div class="panels">
        <wc-panel heading="Balances">
          <wc-balance-list
            .items=${balance?.accounts ?? []}
            .total=${balance?.total}
            ?loading=${store.balance.loading.get()}
            error=${store.balance.error.get()?.message ?? ''}
            @nc-retry=${() => void store.reloadBalance()}
          ></wc-balance-list>
        </wc-panel>

        <wc-panel heading="Cash flow" description=${chart.caption}>
          <wc-bar-chart
            .buckets=${chart.buckets}
            ?loading=${store.cashflow.loading.get()}
            error=${store.cashflow.error.get()?.message ?? ''}
            @nc-retry=${() => void store.reloadCashflow()}
          >
            <wc-empty-state
              slot="empty"
              icon="wc-icon-import"
              heading="No cash flow yet"
              message="Import a statement and the monthly chart fills in."
            ></wc-empty-state>
          </wc-bar-chart>
        </wc-panel>
      </div>
    `;
  }

  render() {
    const store = this.store;
    if (!store) return nothing;

    this.announce(store.pnl.error.get());
    this.announce(store.balance.error.get());
    this.announce(store.cashflow.error.get());
    this.announce(store.flagged.error.get());

    if (store.isEmpty.get()) {
      return html`
        ${this.renderUpdateNotice()}
        <wc-empty-state
          icon="wc-icon-import"
          heading="No books yet"
          message="Import a bank statement to see year-to-date figures, balances and the monthly chart."
        >
          <a slot="actions" href="#/import">Import a statement</a>
        </wc-empty-state>
      `;
    }

    return html`
      ${this.renderUpdateNotice()} ${this.renderToolbar(store)}
      ${this.renderStats(store)} ${this.renderPanels(store)}
    `;
  }
}

export function renderDashboard(ctx: ScreenContext): TemplateResult {
  return html`<nigel-dashboard-screen
    .client=${ctx.client}
  ></nigel-dashboard-screen>`;
}

declare global {
  interface HTMLElementTagNameMap {
    'nigel-dashboard-screen': NigelDashboardScreen;
  }
}

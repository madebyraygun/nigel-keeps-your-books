import { LitElement, html, css, nothing, type TemplateResult } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import '@nigel/ui';
import {
  dispatchNcToast,
  EMPTY_RECONCILE_FORM,
  type NcReconcileChangeDetail,
  type NcReconcileSubmitDetail,
  type ReconcileFormErrors,
  type ReconcileFormValue,
  type ReconciliationHistoryRow,
} from '@nigel/ui';

import { ApiError, type ApiClient } from '../api/index.js';
import type { ReconcileResult } from '../api/types.js';
import { initialAccount, reconcileFailure, toHistoryRows } from './reconcile-data.js';
import type { ScreenContext } from './context.js';
import type { ScreenId } from './registry.js';

/** A verdict and the question it answered. */
interface Reconciled {
  account: string;
  month: string;
  result: ReconcileResult;
}

/**
 * Reconciling a month: the form, the verdict, and what has been checked before.
 *
 * `POST /api/reconcile` records the attempt whichever way it comes out — that
 * record is how the history knows which months have been looked at — so the
 * history is refetched after every submit, not just after a match.
 */
@customElement('nigel-reconcile-screen')
export class NigelReconcileScreen extends LitElement {
  static styles = css`
    :host {
      display: grid;
      gap: var(--wa-space-l, 16px);
      max-width: 52rem;
    }
  `;

  @property({ attribute: false })
  client!: ApiClient;

  @property({ attribute: false })
  params: URLSearchParams = new URLSearchParams();

  @property({ attribute: false })
  navigate?: (screen: ScreenId, params?: URLSearchParams) => void;

  @state() private accounts: string[] = [];
  @state() private form: ReconcileFormValue = EMPTY_RECONCILE_FORM;
  @state() private errors: ReconcileFormErrors = {};
  @state() private formError: string | null = null;
  /**
   * The verdict, kept with the request that produced it.
   *
   * The panel is labelled from the request rather than from the form, so
   * editing the month afterwards cannot relabel February's figures as March's.
   */
  @state() private result: Reconciled | null = null;

  @state() private history: ReconciliationHistoryRow[] = [];
  @state() private historyLoading = true;
  @state() private historyError: string | null = null;

  @state() private accountsLoading = true;
  @state() private accountsError: string | null = null;
  @state() private submitting = false;

  firstUpdated(): void {
    void this.load();
  }

  /**
   * Accounts and history together, but settled separately: a history that
   * fails must not cost the user the form, which is the half that does work.
   */
  private async load(): Promise<void> {
    this.accountsLoading = true;
    try {
      const accounts = await this.client.getAccounts();
      this.accounts = accounts.map((account) => account.name);
      this.accountsError = null;
      this.form = {
        ...this.form,
        account: initialAccount(this.params, this.accounts),
      };
    } catch (error) {
      this.accounts = [];
      this.accountsError =
        error instanceof ApiError ? error.message : 'Could not load the accounts.';
    } finally {
      this.accountsLoading = false;
    }

    await this.loadHistory();
  }

  private loadHistory = async (): Promise<void> => {
    const account = this.form.account;
    if (account === '') {
      this.history = [];
      this.historyLoading = false;
      return;
    }

    this.historyLoading = true;
    try {
      this.history = toHistoryRows(await this.client.getReconciliations({ account }));
      this.historyError = null;
    } catch (error) {
      this.history = [];
      this.historyError =
        error instanceof ApiError
          ? error.message
          : 'Could not load past reconciliations.';
    } finally {
      this.historyLoading = false;
    }
  };

  private handleChange = (event: Event): void => {
    const next = (event as CustomEvent<NcReconcileChangeDetail>).detail.value;
    const accountChanged = next.account !== this.form.account;
    this.form = next;
    // An edit invalidates the last verdict along with the last error: the
    // panel below would otherwise describe a month the form no longer names.
    this.errors = {};
    this.formError = null;
    if (accountChanged) {
      this.result = null;
      this.syncAccountParam(next.account);
      void this.loadHistory();
    }
  };

  /** Keep the deep link honest, so a reload or a share lands on this account. */
  private syncAccountParam(account: string): void {
    if (!this.navigate) return;
    const params = new URLSearchParams();
    if (account !== '') params.set('account', account);
    this.navigate('reconcile', params);
  }

  private handleSubmit = async (event: Event): Promise<void> => {
    if (this.submitting) return;
    const request = (event as CustomEvent<NcReconcileSubmitDetail>).detail;

    this.submitting = true;
    this.errors = {};
    this.formError = null;
    try {
      const result = await this.client.reconcile(request);
      this.result = { account: request.account, month: request.month, result };
      await this.loadHistory();
    } catch (error) {
      // The typed figures stay: this is the one screen where a value was
      // copied off a paper statement, and retyping it is the worst outcome.
      this.result = null;
      const failure = reconcileFailure(error);
      if (failure.field) this.errors = { [failure.field]: failure.message };
      else this.formError = failure.message;
      dispatchNcToast(this, { message: failure.message, variant: 'danger' });
    } finally {
      this.submitting = false;
    }
  };

  private handleRetryAccounts = (): void => {
    void this.load();
  };

  render() {
    return html`
      <wc-panel
        heading="Reconcile an account"
        description="Compare a statement balance against what nigel has recorded. Every check is kept, including the ones that do not balance."
      >
        ${this.renderForm()}
      </wc-panel>

      ${this.result === null
        ? nothing
        : html`
            <wc-reconcile-result
              account=${this.result.account}
              month=${this.result.month}
              ?is-reconciled=${this.result.result.isReconciled}
              .statementBalance=${this.result.result.statementBalance}
              .calculatedBalance=${this.result.result.calculatedBalance}
              .discrepancy=${this.result.result.discrepancy}
            ></wc-reconcile-result>
          `}

      <wc-panel heading="Past reconciliations">
        <wc-reconciliation-history
          .rows=${this.history}
          ?loading=${this.historyLoading}
          .error=${this.historyError}
          @nc-retry=${this.loadHistory}
        ></wc-reconciliation-history>
      </wc-panel>
    `;
  }

  private renderForm(): TemplateResult {
    if (this.accountsLoading) {
      return html`<wc-spinner show-label label="Loading accounts"></wc-spinner>`;
    }

    if (this.accountsError) {
      return html`
        <wc-notice-bar
          variant="danger"
          message=${this.accountsError}
          action-label="Try again"
          @nc-notice-action=${this.handleRetryAccounts}
        ></wc-notice-bar>
      `;
    }

    if (this.accounts.length === 0) {
      return html`
        <wc-empty-state
          icon="wc-icon-account"
          heading="No accounts yet"
          message="No accounts found. Add one first."
        >
          <a slot="actions" href="#/accounts">Go to accounts</a>
        </wc-empty-state>
      `;
    }

    return html`
      ${this.formError
        ? html`<wc-notice-bar variant="danger" message=${this.formError}></wc-notice-bar>`
        : nothing}
      <wc-reconcile-form
        .accounts=${this.accounts}
        .value=${this.form}
        .errors=${this.errors}
        ?busy=${this.submitting}
        @nc-reconcile-change=${this.handleChange}
        @nc-reconcile-submit=${this.handleSubmit}
      ></wc-reconcile-form>
    `;
  }
}

export function renderReconcile(ctx: ScreenContext): TemplateResult {
  return html`
    <nigel-reconcile-screen
      .client=${ctx.client}
      .params=${ctx.params}
      .navigate=${ctx.navigate}
    ></nigel-reconcile-screen>
  `;
}

declare global {
  interface HTMLElementTagNameMap {
    'nigel-reconcile-screen': NigelReconcileScreen;
  }
}

import {
  LitElement,
  html,
  css,
  nothing,
  type PropertyValues,
  type TemplateResult,
} from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import '@nigel/ui';
import {
  confirmDialog,
  dispatchNcToast,
  EMPTY_INVOICE_FORM,
  paymentFormFor,
  validateInvoiceForm,
  validatePaymentForm,
  type InvoiceClientOption,
  type InvoiceFormErrors,
  type InvoiceFormValue,
  type NcInvoiceFormChangeDetail,
  type NcPaymentFormChangeDetail,
  type PaymentFormErrors,
  type PaymentFormValue,
  type SendFailureView,
  type SendStepView,
} from '@nigel/ui';

import { ApiError, type ApiClient } from '../api/index.js';
import { SignalWatcher } from '../mixins/signal-watcher.js';
import { getAppStore, type AppStore } from '../state/app-store.js';
import type {
  AgingReport,
  Client,
  InvoiceDetail,
  InvoiceListRow,
} from '../api/types.js';
import {
  STATUS_FILTERS,
  activeStatusFilter,
  detailLineItems,
  invoiceFormFrom,
  invoiceListParams,
  invoicePatch,
  invoiceTableRows,
  newInvoiceRequest,
  payRequest,
  sendStepViews,
  today,
} from './invoice-data.js';
import {
  invoicingGuardrailMessage,
  sendFailureMessage,
  failedStep,
  completedSteps,
} from './invoicing-errors.js';
import type { ScreenContext } from './context.js';
import type { ScreenId } from './registry.js';

/** Which of the screen's four views the route is asking for. */
type View = 'list' | 'detail' | 'edit' | 'new';

function viewOf(params: URLSearchParams): View {
  if (params.get('new') === '1') return 'new';
  if (params.get('number')) return params.get('edit') === '1' ? 'edit' : 'detail';
  return 'list';
}

function numberOf(params: URLSearchParams): number | null {
  const parsed = Number(params.get('number'));
  return Number.isInteger(parsed) && parsed > 0 ? parsed : null;
}

/**
 * Invoices: the list, one invoice, the editor and the new-invoice form.
 *
 * One screen with four views keyed off `ctx.params`, which is the reports
 * screen's arrangement and for the same reason — the router has no path
 * segments. Filters navigate rather than set state, so they are links and a
 * filtered list is a URL somebody can keep.
 *
 * The **editor is a full view, not a dialog**: `wc-manager-dialog` fits a
 * rule's six fields, and an invoice with eight line items inside one is a
 * scrolling box inside a scrolling page. The send dialog is the opposite
 * departure — it is the one confirmation that survives its own request,
 * because the step trace has nowhere else to be rendered.
 *
 * Every mutation refetches rather than splicing: a send moves a status, a
 * payment moves a status and a balance, and a void changes what the aging
 * strip reports for every other row on the screen.
 */
@customElement('nigel-invoices-screen')
export class NigelInvoicesScreen extends SignalWatcher(LitElement) {
  static styles = css`
    :host {
      display: grid;
      gap: var(--wa-space-l, 16px);
      align-content: start;
      padding: var(--wa-space-l, 16px);
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    header {
      display: flex;
      flex-wrap: wrap;
      align-items: flex-start;
      justify-content: space-between;
      gap: var(--wa-space-m, 12px);
    }

    h2 {
      margin: 0;
      font-size: var(--wa-font-size-l, 18px);
    }

    .actions {
      display: flex;
      flex-wrap: wrap;
      gap: var(--wa-space-s, 8px);
    }

    .filters {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: var(--wa-space-xs, 6px);
      font-size: var(--wa-font-size-s, 13px);
    }

    .filters .label {
      color: var(--wa-color-muted);
    }

    .chip {
      padding: var(--wa-space-2xs, 4px) var(--wa-space-s, 8px);
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-pill, 999px);
      color: inherit;
      text-decoration: none;
    }

    .chip[aria-current='true'] {
      border-color: var(--wa-color-brand);
      color: var(--wa-color-brand);
      font-weight: var(--wa-font-weight-medium, 500);
    }

    .chip:focus-visible {
      outline: 2px solid var(--wa-color-focus);
      outline-offset: 2px;
    }

    .back {
      color: var(--wa-color-brand);
      font-size: var(--wa-font-size-s, 13px);
      text-decoration: none;
    }

    .stack {
      display: grid;
      gap: var(--wa-space-l, 16px);
    }

    .columns {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(18rem, 1fr));
      gap: var(--wa-space-l, 16px);
      align-items: start;
    }

    .error {
      margin: 0;
      color: var(--wa-color-danger);
    }

    .link-list {
      display: grid;
      gap: var(--wa-space-2xs, 4px);
      margin: 0;
      font-size: var(--wa-font-size-s, 13px);
      overflow-wrap: anywhere;
    }

    .link-list dt {
      color: var(--wa-color-muted);
    }

    .link-list dd {
      margin: 0 0 var(--wa-space-xs, 6px);
    }

    .note {
      margin: 0;
      font-size: var(--wa-font-size-s, 13px);
      color: var(--wa-color-muted);
    }
  `;

  @property({ attribute: false })
  client!: ApiClient;

  @property({ attribute: false })
  params: URLSearchParams = new URLSearchParams();

  @property({ attribute: false })
  navigate?: (screen: ScreenId, params?: URLSearchParams) => void;

  @state() private rows: InvoiceListRow[] = [];
  @state() private aging: AgingReport | null = null;
  @state() private detail: InvoiceDetail | null = null;
  @state() private clients: Client[] = [];
  @state() private nextNumber: number | null = null;

  @state() private loading = false;
  /** A load that failed: there is nothing to show, so the whole view is it. */
  @state() private error: string | null = null;
  /**
   * A refused action on data that loaded fine — a 409 on void, a payment the
   * server would not take.
   *
   * Deliberately not `error`: a guardrail is a normal answer, and rendering it
   * through the "that did not load" state would blank the invoice the user is
   * being told something about. It lands in a notice above the detail instead,
   * which is the manager screens' alert-region rule, one screen over.
   */
  @state() private actionError: string | null = null;
  @state() private busy = false;

  @state() private form: InvoiceFormValue = EMPTY_INVOICE_FORM;
  @state() private formErrors: InvoiceFormErrors = {};
  @state() private formError: string | null = null;

  @state() private payment: PaymentFormValue | null = null;
  @state() private paymentErrors: PaymentFormErrors = {};
  @state() private paymentError: string | null = null;

  @state() private sendOpen = false;
  @state() private sendPhase: 'confirm' | 'sending' | 'sent' | 'failed' = 'confirm';
  @state() private sendSteps: SendStepView[] = [];
  @state() private sendFailure: SendFailureView | null = null;
  @state() private sentUrl = '';

  private appStore: AppStore = getAppStore();

  /**
   * The route the shown data answers, so a re-render does not refetch.
   *
   * Null rather than the empty string, because the empty string is a real
   * route — the unfiltered list — and a sentinel that collides with a value
   * would leave that one view never loading.
   */
  private loadedKey: string | null = null;

  /** Drops an answer whose request is no longer the current one. */
  private loadSeq = 0;

  private get invoicing() {
    return this.appStore.status.get()?.invoicing;
  }

  private get pdfExport(): boolean {
    return this.appStore.status.get()?.pdfExport ?? true;
  }

  willUpdate(changed: PropertyValues<this>): void {
    if (!changed.has('params')) return;
    const key = this.params.toString();
    if (key === this.loadedKey) return;
    void this.load(key);
  }

  private async load(key: string, keepActionError = false): Promise<void> {
    const seq = (this.loadSeq += 1);
    const view = viewOf(this.params);
    const number = numberOf(this.params);
    this.loading = true;
    this.error = null;
    // A refetch that a refusal triggered keeps the refusal; a route change
    // drops it, because it was about the invoice being left behind.
    if (!keepActionError) this.actionError = null;

    try {
      if (view === 'list') {
        const [rows, aging] = await Promise.all([
          this.client.getInvoices(invoiceListParams(this.params)),
          this.agingOrNull(),
        ]);
        if (seq !== this.loadSeq) return;
        this.rows = rows;
        this.aging = aging;
      } else if (view === 'new') {
        const [clients, next] = await Promise.all([
          this.client.getClients(),
          this.client.getNextInvoiceNumber(),
        ]);
        if (seq !== this.loadSeq) return;
        this.clients = clients;
        this.nextNumber = next.number;
        this.detail = null;
        this.form = { ...EMPTY_INVOICE_FORM, issueDate: today() };
        this.formErrors = {};
        this.formError = null;
      } else if (number !== null) {
        const detail = await this.client.getInvoice(number);
        if (seq !== this.loadSeq) return;
        this.detail = detail;
        if (view === 'edit') {
          this.clients = await this.client.getClients();
          if (seq !== this.loadSeq) return;
          this.form = invoiceFormFrom(detail);
          this.formErrors = {};
          this.formError = null;
        }
      }
      this.loadedKey = key;
    } catch (cause) {
      if (seq !== this.loadSeq) return;
      this.loadedKey = null;
      this.error =
        cause instanceof ApiError ? cause.message : 'Could not load the invoices.';
    } finally {
      if (seq === this.loadSeq) this.loading = false;
    }
  }

  /**
   * The aging strip is secondary to the list it sits above.
   *
   * A failure there renders as no strip rather than as no invoices — the
   * dashboard's per-card reasoning, one card down.
   */
  private async agingOrNull(): Promise<AgingReport | null> {
    try {
      return await this.client.getAging();
    } catch {
      return null;
    }
  }

  /** Refetch whatever the current route shows, after a write. */
  private async refresh(keepActionError = false): Promise<void> {
    this.loadedKey = null;
    await this.load(this.params.toString(), keepActionError);
  }

  private go(changes: Record<string, string | null>): void {
    const next = new URLSearchParams(this.params);
    for (const [key, value] of Object.entries(changes)) {
      if (value === null) next.delete(key);
      else next.set(key, value);
    }
    this.navigate?.('invoices', next);
  }

  private href(changes: Record<string, string | null>): string {
    const next = new URLSearchParams(this.params);
    for (const [key, value] of Object.entries(changes)) {
      if (value === null) next.delete(key);
      else next.set(key, value);
    }
    const query = next.toString();
    return `#/invoices${query ? `?${query}` : ''}`;
  }

  // -- writes ---------------------------------------------------------------

  private handleFormChange = (event: Event): void => {
    this.form = (event as CustomEvent<NcInvoiceFormChangeDetail>).detail.value;
  };

  private handleCreate = async (): Promise<void> => {
    if (this.busy) return;
    const errors = validateInvoiceForm(this.form);
    this.formErrors = errors;
    if (Object.keys(errors).length > 0) return;

    const request = newInvoiceRequest(this.form);
    if (!request) return;

    this.busy = true;
    this.formError = null;
    try {
      const created = await this.client.createInvoice(request);
      this.go({ new: null, number: String(created.number) });
    } catch (error) {
      this.formError = invoicingGuardrailMessage(error, 'invoice');
    } finally {
      this.busy = false;
    }
  };

  private handleSaveEdit = async (): Promise<void> => {
    const detail = this.detail;
    if (!detail || this.busy) return;

    const errors = validateInvoiceForm(this.form);
    this.formErrors = errors;
    if (Object.keys(errors).length > 0) return;

    const patch = invoicePatch(detail, this.form);
    // An all-absent PATCH is a 400: a save with nothing changed is a close.
    if (Object.keys(patch).length === 0) {
      this.go({ edit: null });
      return;
    }

    this.busy = true;
    this.formError = null;
    try {
      await this.client.updateInvoice(detail.number, patch);
      this.go({ edit: null });
    } catch (error) {
      this.formError = invoicingGuardrailMessage(error, 'invoice');
    } finally {
      this.busy = false;
    }
  };

  private handleVoid = async (): Promise<void> => {
    const detail = this.detail;
    if (!detail) return;

    const confirmed = await confirmDialog({
      heading: `Void invoice #${detail.number}?`,
      message:
        'A void invoice cannot be edited, sent or paid, and voiding does not take down a page that has already been published or deactivate its payment link.',
      confirmLabel: 'Void invoice',
      variant: 'danger',
    });
    if (!confirmed) return;

    this.busy = true;
    try {
      await this.client.voidInvoice(detail.number);
      this.actionError = null;
      await this.refresh();
    } catch (error) {
      // `confirmDialog()` has already resolved and removed itself, so the
      // refusal lands above the invoice it is about — never in place of it.
      // The refetch stands because a refusal can mean the view is stale:
      // `already_void` is somebody else having voided it in another tab.
      this.actionError = invoicingGuardrailMessage(error, 'invoice');
      await this.refresh(true);
    } finally {
      this.busy = false;
    }
  };

  private dismissActionError = (): void => {
    this.actionError = null;
  };

  private openPayment = (): void => {
    const detail = this.detail;
    if (!detail) return;
    this.payment = paymentFormFor(detail.balance, today());
    this.paymentErrors = {};
    this.paymentError = null;
  };

  private closePayment = (): void => {
    this.payment = null;
    this.paymentErrors = {};
    this.paymentError = null;
  };

  private handlePaymentChange = (event: Event): void => {
    this.payment = (event as CustomEvent<NcPaymentFormChangeDetail>).detail.value;
  };

  private handleRecordPayment = async (): Promise<void> => {
    const detail = this.detail;
    const payment = this.payment;
    if (!detail || !payment || this.busy) return;

    const errors = validatePaymentForm(payment);
    this.paymentErrors = errors;
    if (Object.keys(errors).length > 0) return;

    this.busy = true;
    this.paymentError = null;
    try {
      await this.client.payInvoice(detail.number, payRequest(payment));
      this.closePayment();
      await this.refresh();
    } catch (error) {
      this.paymentError = invoicingGuardrailMessage(error, 'invoice');
    } finally {
      this.busy = false;
    }
  };

  private openSend = (): void => {
    this.sendOpen = true;
    this.sendPhase = 'confirm';
    this.sendSteps = [];
    this.sendFailure = null;
    this.sentUrl = '';
  };

  private closeSend = (): void => {
    this.sendOpen = false;
    // The status, the balance and the aging strip all moved if it went out.
    void this.refresh();
  };

  private handleSend = async (): Promise<void> => {
    const detail = this.detail;
    if (!detail) return;

    this.sendPhase = 'sending';
    this.sendFailure = null;
    this.sendSteps = sendStepViews({ running: 'config' });

    try {
      const result = await this.client.sendInvoice(detail.number);
      this.sendSteps = sendStepViews({ completed: result.steps });
      this.sentUrl = result.publicUrl;
      this.sendPhase = 'sent';
      this.detail = result.invoice;
    } catch (error) {
      this.sendSteps = sendStepViews({
        completed: completedSteps(error),
        failed: failedStep(error),
      });
      this.sendFailure = sendFailureMessage(error, detail.number);
      this.sendPhase = 'failed';
    }
  };

  private handleSync = async (): Promise<void> => {
    if (this.busy) return;
    this.busy = true;
    try {
      const result = await this.client.syncInvoices();
      const failed = result.failures.length;
      dispatchNcToast(this, {
        variant: failed > 0 ? 'danger' : 'success',
        message:
          `Checked ${result.invoicesChecked}, recorded ${result.recorded}.` +
          (failed > 0
            ? ` ${failed} could not be checked: ${result.failures
                .map((failure) => `#${failure.number}`)
                .join(', ')}.`
            : ''),
      });
      await this.refresh();
    } catch (error) {
      dispatchNcToast(this, {
        variant: 'danger',
        message: invoicingGuardrailMessage(error, 'invoice'),
      });
    } finally {
      this.busy = false;
    }
  };

  // -- rendering ------------------------------------------------------------

  render() {
    if (this.error) {
      return html`
        <wc-empty-state icon="wc-icon-invoice" heading="That did not load">
          <p class="error">${this.error}</p>
        </wc-empty-state>
      `;
    }

    if (this.loading) {
      return html`<wc-spinner size="l" show-label label="Loading invoices"></wc-spinner>`;
    }

    switch (viewOf(this.params)) {
      case 'new':
        return this.renderEditor('create');
      case 'edit':
        return this.renderEditor('edit');
      case 'detail':
        return this.renderDetail();
      default:
        return this.renderList();
    }
  }

  private renderList() {
    const syncConfigured = this.invoicing?.syncConfigured ?? false;

    return html`
      <header>
        <h2>Invoices</h2>
        <div class="actions">
          <wa-button
            data-sync
            appearance="outlined"
            ?disabled=${!syncConfigured || this.busy}
            title=${syncConfigured
              ? 'Pull paid Stripe checkouts and record them'
              : 'stripe_secret_key is not set'}
            @click=${this.handleSync}
          >
            Sync now
          </wa-button>
          <wa-button data-new variant="brand" @click=${() => this.go({ new: '1' })}>
            <wc-icon-plus slot="start"></wc-icon-plus>
            New invoice
          </wa-button>
        </div>
      </header>

      ${syncConfigured
        ? nothing
        : html`<p class="note" data-sync-note>
            Sync needs stripe_secret_key, which is not set.
          </p>`}
      ${this.aging
        ? html`<wc-aging-bars
            .buckets=${this.aging.buckets}
            .total=${this.aging.outstanding}
            as-of=${this.aging.asOf}
            href="#/reports?report=aging"
          ></wc-aging-bars>`
        : nothing}

      <div class="filters">
        <span class="label">Status</span>
        ${STATUS_FILTERS.map((filter) => {
          const active = activeStatusFilter(this.params) === filter.value;
          return html`<a
            class="chip"
            data-filter=${filter.value}
            aria-current=${active ? 'true' : nothing}
            href=${this.href({ status: filter.value === 'all' ? null : filter.value })}
            >${filter.label}</a
          >`;
        })}
        ${this.params.get('clientId')
          ? html`<a class="chip" data-clear-client href=${this.href({ clientId: null })}
              >Clear client filter</a
            >`
          : nothing}
      </div>

      <wc-invoice-table
        .rows=${invoiceTableRows(this.rows)}
        empty-message="No invoices yet — start one with New invoice."
      ></wc-invoice-table>
    `;
  }

  private renderDetail() {
    const detail = this.detail;
    if (!detail) return nothing;

    const sendBlock = this.sendBlockReason(detail);

    return html`
      <a class="back" href=${this.href({ number: null, edit: null })}>← All invoices</a>

      ${this.actionError
        ? html`<wc-notice-bar
            variant="danger"
            data-action-error
            message=${this.actionError}
            action-label="Dismiss"
            @nc-notice-action=${this.dismissActionError}
          ></wc-notice-bar>`
        : nothing}

      <wc-invoice-summary
        .number=${detail.number}
        status=${detail.status}
        .clientName=${detail.client?.name ?? null}
        .total=${detail.total}
        .balance=${detail.balance}
        .currency=${detail.currency}
        .issueDate=${detail.issueDate}
        .dueDate=${detail.dueDate}
      ></wc-invoice-summary>

      <div class="actions">
        <wa-button
          data-send
          variant="brand"
          ?disabled=${!detail.canSend || sendBlock !== '' || this.busy}
          title=${sendBlock}
          @click=${this.openSend}
        >
          Send…
        </wa-button>
        <wa-button
          data-pay
          appearance="outlined"
          ?disabled=${!detail.canPay || this.busy}
          @click=${this.openPayment}
        >
          Record payment…
        </wa-button>
        <wa-button
          data-edit
          appearance="outlined"
          ?disabled=${!detail.canEdit || this.busy}
          @click=${() => this.go({ edit: '1' })}
        >
          Edit
        </wa-button>
        <wa-button
          data-void
          appearance="outlined"
          variant="danger"
          ?disabled=${!detail.canVoid || this.busy}
          @click=${this.handleVoid}
        >
          Void…
        </wa-button>
      </div>

      ${detail.canSend
        ? nothing
        : html`<p class="note" data-send-note>
            ${this.sendUnavailableNote(detail)}
          </p>`}

      <wc-panel heading="Line items">
        <wc-line-items
          readonly
          caption="Line items"
          caption-hidden
          .items=${detailLineItems(detail)}
          .total=${detail.total}
        ></wc-line-items>
      </wc-panel>

      <div class="columns">
        <wc-panel heading="Payments">
          <wc-payment-list .payments=${detail.payments}></wc-payment-list>
        </wc-panel>
        <wc-panel heading="Published">${this.renderPublished(detail)}</wc-panel>
      </div>

      <wc-invoice-preview
        .src=${this.client.invoicePreviewUrl(detail.number, 'html')}
        .pdfSrc=${this.client.invoicePreviewUrl(detail.number, 'pdf')}
        .pdfAvailable=${this.pdfExport}
        .missing=${this.invoicing?.missing ?? []}
      ></wc-invoice-preview>

      ${this.renderPaymentDialog(detail)} ${this.renderSendDialog(detail, sendBlock)}
    `;
  }

  private renderPublished(detail: InvoiceDetail) {
    if (!detail.publicUrl && !detail.stripePaymentLinkUrl) {
      return html`<p class="note">
        Nothing published yet. Sending creates the page and the payment link.
      </p>`;
    }

    return html`
      <dl class="link-list">
        ${detail.publicUrl
          ? html`<dt>Invoice page</dt>
              <dd data-public-url>
                <a href=${detail.publicUrl} target="_blank" rel="noreferrer"
                  >${detail.publicUrl}</a
                >
              </dd>`
          : nothing}
        ${detail.stripePaymentLinkUrl
          ? html`<dt>Payment link</dt>
              <dd data-pay-link>
                <a href=${detail.stripePaymentLinkUrl} target="_blank" rel="noreferrer"
                  >${detail.stripePaymentLinkUrl}</a
                >
              </dd>`
          : nothing}
      </dl>
    `;
  }

  /** Why Send is unavailable, in one sentence, or empty when it is available. */
  private sendBlockReason(detail: InvoiceDetail): string {
    if (detail.client && detail.client.email === null) {
      return `${detail.client.name} has no email address. Add one on the client before sending.`;
    }
    const invoicing = this.invoicing;
    if (invoicing && !invoicing.sendConfigured) {
      return invoicing.missing.length > 0
        ? `Sending needs ${invoicing.missing.join(', ')}, which ${
            invoicing.missing.length === 1 ? 'is' : 'are'
          } not set.`
        : 'Sending is not configured yet.';
    }
    return '';
  }

  private sendUnavailableNote(detail: InvoiceDetail): string {
    const blocked = this.sendBlockReason(detail);
    if (blocked) return blocked;
    if (detail.status === 'void') return 'A void invoice cannot be sent.';
    if (detail.balance <= 0) return 'This invoice has nothing left to bill for.';
    return 'This invoice cannot be sent.';
  }

  private renderPaymentDialog(detail: InvoiceDetail) {
    const payment = this.payment;
    if (!payment) return nothing;

    return html`
      <wc-manager-dialog
        open
        heading=${`Record a payment against #${detail.number}`}
        confirm-label="Record payment"
        ?busy=${this.busy}
        .error=${this.paymentError}
        @nc-manager-save=${this.handleRecordPayment}
        @nc-manager-cancel=${this.closePayment}
      >
        <wc-payment-form
          .value=${payment}
          .errors=${this.paymentErrors}
          .balance=${detail.balance}
          ?disabled=${this.busy}
          @nc-payment-form-change=${this.handlePaymentChange}
        ></wc-payment-form>
      </wc-manager-dialog>
    `;
  }

  private renderSendDialog(detail: InvoiceDetail, blocked: string) {
    if (!this.sendOpen) return nothing;

    return html`
      <wc-send-dialog
        open
        .number=${detail.number}
        .total=${detail.total}
        .currency=${detail.currency}
        .recipient=${detail.client?.email ?? ''}
        .publishHost=${hostOf(detail.publicUrl)}
        phase=${this.sendPhase}
        .steps=${this.sendSteps}
        .failure=${this.sendFailure}
        .publicUrl=${this.sentUrl}
        .blocked=${blocked}
        @nc-send-confirm=${this.handleSend}
        @nc-send-close=${this.closeSend}
      ></wc-send-dialog>
    `;
  }

  private renderEditor(mode: 'create' | 'edit') {
    const detail = this.detail;
    const heading =
      mode === 'create'
        ? this.nextNumber
          ? `New invoice #${this.nextNumber}`
          : 'New invoice'
        : `Edit invoice #${detail?.number ?? ''}`;

    return html`
      <a
        class="back"
        href=${mode === 'create'
          ? this.href({ new: null })
          : this.href({ edit: null })}
        >← Back</a
      >

      <header>
        <h2>${heading}</h2>
        <div class="actions">
          <wa-button
            data-cancel
            appearance="outlined"
            ?disabled=${this.busy}
            @click=${() => this.go(mode === 'create' ? { new: null } : { edit: null })}
          >
            Cancel
          </wa-button>
          <wa-button
            data-save
            variant="brand"
            ?disabled=${this.busy}
            @click=${mode === 'create' ? this.handleCreate : this.handleSaveEdit}
          >
            ${this.busy ? 'Saving…' : mode === 'create' ? 'Create draft' : 'Save changes'}
          </wa-button>
        </div>
      </header>

      ${this.formError
        ? html`<wc-notice-bar
            variant="danger"
            data-form-error
            message=${this.formError}
          ></wc-notice-bar>`
        : nothing}

      <wc-invoice-form
        mode=${mode}
        .value=${this.form}
        .errors=${this.formErrors}
        .clients=${this.clients.map(
          (client): InvoiceClientOption => ({
            id: client.id,
            name: client.name,
            email: client.email,
          }),
        )}
        ?disabled=${this.busy}
        @nc-invoice-form-change=${this.handleFormChange}
      ></wc-invoice-form>
    `;
  }
}

/** The host an already-published invoice lives on, for the send confirmation. */
function hostOf(url: string | null): string {
  if (!url) return '';
  try {
    return new URL(url).host;
  } catch {
    return '';
  }
}

export function renderInvoices(ctx: ScreenContext): TemplateResult {
  return html`
    <nigel-invoices-screen
      .client=${ctx.client}
      .params=${ctx.params}
      .navigate=${ctx.navigate}
    ></nigel-invoices-screen>
  `;
}

declare global {
  interface HTMLElementTagNameMap {
    'nigel-invoices-screen': NigelInvoicesScreen;
  }
}

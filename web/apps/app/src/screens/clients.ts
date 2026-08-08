import { LitElement, html, css, nothing, type TemplateResult } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import '@nigel/ui';
import {
  confirmDialog,
  EMPTY_CLIENT_FORM,
  validateClientForm,
  type ClientFormErrors,
  type ClientFormValue,
  type ManagerAction,
  type ManagerColumn,
  type ManagerRow,
  type NcClientFormChangeDetail,
  type NcManagerActionDetail,
} from '@nigel/ui';

import { ApiError, type ApiClient } from '../api/index.js';
import type { Client } from '../api/types.js';
import { clientFormFrom, clientPatch, newClientRequest } from './invoice-data.js';
import {
  invoicingGuardrailAction,
  invoicingGuardrailMessage,
} from './invoicing-errors.js';
import type { ScreenContext } from './context.js';
import type { ScreenId } from './registry.js';

const COLUMNS: ManagerColumn[] = [
  { key: 'name', label: 'Name' },
  { key: 'email', label: 'Email' },
  { key: 'billingAddress', label: 'Billing address' },
];

const ACTIONS: ManagerAction[] = [
  { name: 'invoices', label: 'Invoices' },
  { name: 'edit', label: 'Edit', icon: 'wc-icon-edit' },
  { name: 'delete', label: 'Delete', icon: 'wc-icon-trash', variant: 'danger' },
];

interface Editor {
  mode: 'create' | 'edit';
  /** The client being edited; absent when creating. */
  id?: number;
  value: ClientFormValue;
}

/**
 * The clients manager — `client_manager.rs` on the web, plus a delete.
 *
 * There is no invoice-count or outstanding column: `GET /api/clients` answers
 * bare `Client` rows, and a screen may not invent an endpoint or fan out one
 * request per row to fake one. That is the accounts precedent exactly, and the
 * count appears where it is actually actionable — in the message a blocked
 * delete comes back with, which also links to the invoices behind it.
 *
 * The delete is the one operation the CLI has never had. It refuses a client
 * with invoices of any status, which is most of them, so the button is honest
 * only because the refusal explains itself and points somewhere.
 */
@customElement('nigel-clients-screen')
export class NigelClientsScreen extends LitElement {
  static styles = css`
    :host {
      display: block;
    }
  `;

  @property({ attribute: false })
  client!: ApiClient;

  @property({ attribute: false })
  navigate?: (screen: ScreenId, params?: URLSearchParams) => void;

  @state() private clients: Client[] = [];
  @state() private loading = true;
  @state() private error: string | null = null;
  @state() private errorAction: { label: string; run: () => void } | null = null;
  @state() private editor: Editor | null = null;
  @state() private formErrors: ClientFormErrors = {};
  @state() private saving = false;
  @state() private dialogError: string | null = null;
  @state() private busyId: number | null = null;

  firstUpdated(): void {
    void this.load();
  }

  private async load(): Promise<void> {
    this.loading = true;
    try {
      this.clients = await this.client.getClients();
      this.error = null;
      this.errorAction = null;
    } catch (error) {
      this.clients = [];
      this.error =
        error instanceof ApiError ? error.message : 'Could not load the clients.';
      this.errorAction = { label: 'Try again', run: () => void this.load() };
    } finally {
      this.loading = false;
    }
  }

  private get rows(): ManagerRow[] {
    return this.clients.map((client) => ({
      id: client.id,
      label: client.name,
      cells: [client.name, client.email, client.billingAddress],
    }));
  }

  // -- editing --------------------------------------------------------------

  private openCreate = (): void => {
    this.editor = { mode: 'create', value: EMPTY_CLIENT_FORM };
    this.formErrors = {};
    this.dialogError = null;
  };

  private closeEditor = (): void => {
    this.editor = null;
    this.formErrors = {};
    this.dialogError = null;
  };

  private handleFormChange = (event: Event): void => {
    if (!this.editor) return;
    const detail = (event as CustomEvent<NcClientFormChangeDetail>).detail;
    this.editor = { ...this.editor, value: detail.value };
  };

  private handleAction = (event: Event): void => {
    const { action, id } = (event as CustomEvent<NcManagerActionDetail>).detail;
    const client = this.clients.find((candidate) => candidate.id === id);
    if (!client) return;

    if (action === 'invoices') {
      this.navigate?.('invoices', new URLSearchParams({ clientId: String(id) }));
      return;
    }
    if (action === 'edit') {
      this.editor = { mode: 'edit', id, value: clientFormFrom(client) };
      this.formErrors = {};
      this.dialogError = null;
      return;
    }
    if (action === 'delete') void this.confirmDelete(client);
  };

  private handleSave = async (): Promise<void> => {
    const editor = this.editor;
    if (!editor || this.saving) return;

    const errors = validateClientForm(editor.value);
    this.formErrors = errors;
    if (Object.keys(errors).length > 0) return;

    this.saving = true;
    this.dialogError = null;
    try {
      if (editor.mode === 'create') {
        await this.client.createClient(newClientRequest(editor.value));
      } else if (editor.id !== undefined) {
        const current = this.clients.find((candidate) => candidate.id === editor.id);
        const patch = current ? clientPatch(current, editor.value) : {};
        // An all-absent PATCH is a 400: a save with nothing changed is a close.
        if (Object.keys(patch).length === 0) {
          this.closeEditor();
          return;
        }
        await this.client.updateClient(editor.id, patch);
      }

      this.closeEditor();
      await this.load();
    } catch (error) {
      this.dialogError = invoicingGuardrailMessage(error, 'client');
    } finally {
      this.saving = false;
    }
  };

  // -- deleting -------------------------------------------------------------

  private async confirmDelete(client: Client): Promise<void> {
    const confirmed = await confirmDialog({
      heading: 'Delete client',
      message: `Delete “${client.name}”? Nigel refuses this while any invoice bills them, of any status.`,
      confirmLabel: 'Delete',
      variant: 'danger',
    });
    if (!confirmed) return;

    this.busyId = client.id;
    try {
      await this.client.deleteClient(client.id);
      this.error = null;
      this.errorAction = null;
      await this.load();
    } catch (error) {
      // `confirmDialog()` has already resolved and removed itself, so the
      // refusal lands in the layout's alert region rather than in a toast that
      // would take the count away before it had been read.
      this.error = invoicingGuardrailMessage(error, 'client');
      const action = invoicingGuardrailAction(error, client.id);
      this.errorAction = action
        ? { label: action.label, run: () => this.navigate?.('invoices', action.params) }
        : null;
    } finally {
      this.busyId = null;
    }
  }

  private handleErrorAction = (): void => {
    this.errorAction?.run();
  };

  private handleErrorDismiss = (): void => {
    this.error = null;
    this.errorAction = null;
  };

  // -- rendering ------------------------------------------------------------

  render() {
    const empty = !this.loading && this.clients.length === 0;

    return html`
      <wc-manager-layout
        heading="Clients"
        .count=${this.loading ? null : this.clients.length}
        add-label="Add client"
        ?busy=${this.loading}
        ?empty=${empty}
        .error=${this.error}
        error-action-label=${this.errorAction?.label ?? ''}
        @nc-manager-add=${this.openCreate}
        @nc-manager-error-action=${this.handleErrorAction}
        @nc-manager-error-dismiss=${this.handleErrorDismiss}
      >
        <wc-manager-table
          caption="Clients"
          .columns=${COLUMNS}
          .rows=${this.rows}
          .actions=${ACTIONS}
          .busyId=${this.busyId}
          @nc-manager-action=${this.handleAction}
        ></wc-manager-table>

        <wc-empty-state
          slot="empty"
          icon="wc-icon-clients"
          heading="No clients yet"
          message="Add one to start invoicing."
        ></wc-empty-state>

        ${this.renderEditor()}
      </wc-manager-layout>
    `;
  }

  private renderEditor(): TemplateResult | typeof nothing {
    const editor = this.editor;
    if (!editor) return nothing;

    const creating = editor.mode === 'create';

    return html`
      <wc-manager-dialog
        slot="overlay"
        open
        heading=${creating ? 'Add client' : 'Edit client'}
        confirm-label=${creating ? 'Add client' : 'Save'}
        ?busy=${this.saving}
        .error=${this.dialogError}
        @nc-manager-save=${this.handleSave}
        @nc-manager-cancel=${this.closeEditor}
      >
        <wc-client-form
          .value=${editor.value}
          .errors=${this.formErrors}
          ?disabled=${this.saving}
          @nc-client-form-change=${this.handleFormChange}
        ></wc-client-form>
      </wc-manager-dialog>
    `;
  }
}

export function renderClients(ctx: ScreenContext): TemplateResult {
  return html`
    <nigel-clients-screen
      .client=${ctx.client}
      .navigate=${ctx.navigate}
    ></nigel-clients-screen>
  `;
}

declare global {
  interface HTMLElementTagNameMap {
    'nigel-clients-screen': NigelClientsScreen;
  }
}

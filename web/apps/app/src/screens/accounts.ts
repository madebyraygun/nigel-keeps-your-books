import { LitElement, html, css, nothing, type TemplateResult } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import '@nigel/ui';
import {
  accountTypeLabel,
  confirmDialog,
  EMPTY_ACCOUNT_FORM,
  validateAccountForm,
  type AccountFormErrors,
  type AccountFormValue,
  type ManagerAction,
  type ManagerColumn,
  type ManagerRow,
  type NcAccountFormChangeDetail,
  type NcManagerActionDetail,
  type WcAccountFormMode,
} from '@nigel/ui';

import { ApiError, type ApiClient } from '../api/index.js';
import type { Account } from '../api/types.js';
import { guardrailMessage } from './manager-errors.js';
import type { ScreenContext } from './context.js';

const COLUMNS: ManagerColumn[] = [
  { key: 'name', label: 'Name' },
  { key: 'accountType', label: 'Type' },
  { key: 'institution', label: 'Institution' },
  { key: 'lastFour', label: 'Last four' },
];

const ACTIONS: ManagerAction[] = [
  { name: 'rename', label: 'Rename', icon: 'wc-icon-edit' },
  { name: 'delete', label: 'Delete', icon: 'wc-icon-trash', variant: 'danger' },
];

interface Editor {
  mode: WcAccountFormMode;
  /** The account being renamed; absent when creating. */
  id?: number;
  value: AccountFormValue;
}

/** Empty means "no value", which on the wire is null rather than "". */
function orNull(value: string): string | null {
  const trimmed = value.trim();
  return trimmed === '' ? null : trimmed;
}

function toFormValue(account: Account): AccountFormValue {
  return {
    name: account.name,
    accountType: account.accountType,
    institution: account.institution ?? '',
    lastFour: account.lastFour ?? '',
  };
}

/**
 * The accounts manager — `account_manager.rs` on the web.
 *
 * There is no transaction-count column: `GET /api/accounts` does not carry one,
 * and a screen may not invent an endpoint. The number appears where it is
 * actually actionable, in the message a blocked delete comes back with.
 */
@customElement('nigel-accounts-screen')
export class NigelAccountsScreen extends LitElement {
  static styles = css`
    :host {
      display: block;
    }
  `;

  @property({ attribute: false })
  client!: ApiClient;

  @state() private accounts: Account[] = [];
  @state() private loading = true;
  @state() private error: string | null = null;
  @state() private retryable = false;
  @state() private editor: Editor | null = null;
  @state() private formErrors: AccountFormErrors = {};
  @state() private saving = false;
  @state() private dialogError: string | null = null;
  @state() private busyId: number | null = null;

  firstUpdated(): void {
    void this.load();
  }

  private async load(): Promise<void> {
    this.loading = true;
    try {
      this.accounts = await this.client.getAccounts();
      this.error = null;
      this.retryable = false;
    } catch (error) {
      this.accounts = [];
      this.retryable = true;
      this.error =
        error instanceof ApiError ? error.message : 'Could not load the accounts.';
    } finally {
      this.loading = false;
    }
  }

  private get rows(): ManagerRow[] {
    return this.accounts.map((account) => ({
      id: account.id,
      label: account.name,
      cells: [
        account.name,
        accountTypeLabel(account.accountType),
        account.institution,
        account.lastFour,
      ],
    }));
  }

  // -- editing --------------------------------------------------------------

  private openCreate = (): void => {
    this.editor = { mode: 'create', value: EMPTY_ACCOUNT_FORM };
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
    const detail = (event as CustomEvent<NcAccountFormChangeDetail>).detail;
    this.editor = { ...this.editor, value: detail.value };
  };

  private handleAction = (event: Event): void => {
    const { action, id } = (event as CustomEvent<NcManagerActionDetail>).detail;
    const account = this.accounts.find((candidate) => candidate.id === id);
    if (!account) return;

    if (action === 'rename') {
      this.editor = { mode: 'rename', id, value: toFormValue(account) };
      this.formErrors = {};
      this.dialogError = null;
      return;
    }
    if (action === 'delete') void this.confirmDelete(account);
  };

  private handleSave = async (): Promise<void> => {
    const editor = this.editor;
    if (!editor || this.saving) return;

    const errors = validateAccountForm(editor.value);
    this.formErrors = errors;
    if (Object.keys(errors).length > 0) return;

    this.saving = true;
    this.dialogError = null;
    try {
      if (editor.mode === 'create') {
        await this.client.createAccount({
          name: editor.value.name.trim(),
          accountType: editor.value.accountType,
          institution: orNull(editor.value.institution),
          lastFour: orNull(editor.value.lastFour),
        });
      } else if (editor.id !== undefined) {
        const current = this.accounts.find((account) => account.id === editor.id);
        // A rename to the same name is a request that can only fail on itself.
        if (current && current.name === editor.value.name.trim()) {
          this.closeEditor();
          return;
        }
        await this.client.renameAccount(editor.id, { name: editor.value.name.trim() });
      }

      this.closeEditor();
      await this.load();
    } catch (error) {
      this.dialogError = guardrailMessage(error, 'account');
    } finally {
      this.saving = false;
    }
  };

  // -- deleting -------------------------------------------------------------

  private async confirmDelete(account: Account): Promise<void> {
    const confirmed = await confirmDialog({
      heading: 'Delete account',
      message: `Delete “${account.name}”? Its reconciliation history goes with it. Imported files stay recorded, so duplicate detection keeps working.`,
      confirmLabel: 'Delete',
      variant: 'danger',
    });
    if (!confirmed) return;

    this.busyId = account.id;
    try {
      await this.client.deleteAccount(account.id);
      this.error = null;
      await this.load();
    } catch (error) {
      // The confirm dialog has already resolved and removed itself, so the
      // refusal lands in the screen's own alert region rather than in a toast
      // that would take the count away before it had been read.
      this.error = guardrailMessage(error, 'account');
      this.retryable = false;
    } finally {
      this.busyId = null;
    }
  }

  private handleErrorAction = (): void => {
    if (this.retryable) void this.load();
  };

  private handleErrorDismiss = (): void => {
    this.error = null;
  };

  // -- rendering ------------------------------------------------------------

  render() {
    const empty = !this.loading && this.accounts.length === 0;

    return html`
      <wc-manager-layout
        heading="Accounts"
        .count=${this.loading ? null : this.accounts.length}
        add-label="Add account"
        ?busy=${this.loading}
        ?empty=${empty}
        .error=${this.error}
        error-action-label=${this.retryable ? 'Try again' : ''}
        @nc-manager-add=${this.openCreate}
        @nc-manager-error-action=${this.handleErrorAction}
        @nc-manager-error-dismiss=${this.handleErrorDismiss}
      >
        <wc-manager-table
          caption="Accounts"
          .columns=${COLUMNS}
          .rows=${this.rows}
          .actions=${ACTIONS}
          .busyId=${this.busyId}
          @nc-manager-action=${this.handleAction}
        ></wc-manager-table>

        <wc-empty-state
          slot="empty"
          icon="wc-icon-account"
          heading="No accounts yet"
          message="Accounts are created when you import a statement, or you can add one here."
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
        heading=${creating ? 'Add account' : 'Rename account'}
        confirm-label=${creating ? 'Add account' : 'Save'}
        ?busy=${this.saving}
        .error=${this.dialogError}
        @nc-manager-save=${this.handleSave}
        @nc-manager-cancel=${this.closeEditor}
      >
        <wc-account-form
          mode=${editor.mode}
          .value=${editor.value}
          .errors=${this.formErrors}
          ?disabled=${this.saving}
          @nc-account-form-change=${this.handleFormChange}
        ></wc-account-form>
      </wc-manager-dialog>
    `;
  }
}

export function renderAccounts(ctx: ScreenContext): TemplateResult {
  return html`<nigel-accounts-screen .client=${ctx.client}></nigel-accounts-screen>`;
}

declare global {
  interface HTMLElementTagNameMap {
    'nigel-accounts-screen': NigelAccountsScreen;
  }
}

import { html, type TemplateResult } from 'lit';
import '@nigel/ui';

export function renderAccounts(): TemplateResult {
  return html`
    <wc-empty-state
      icon="wc-icon-account"
      heading="Accounts"
      message="Adding, renaming, and deleting accounts arrives in task 31.16."
    ></wc-empty-state>
  `;
}

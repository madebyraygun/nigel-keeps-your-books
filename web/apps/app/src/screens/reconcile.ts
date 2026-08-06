import { html, type TemplateResult } from 'lit';
import '@nigel/ui';

export function renderReconcile(): TemplateResult {
  return html`
    <wc-empty-state
      icon="wc-icon-reconcile"
      heading="Reconcile"
      message="Monthly account reconciliation arrives in task 31.17."
    ></wc-empty-state>
  `;
}

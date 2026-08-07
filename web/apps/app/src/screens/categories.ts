import { html, type TemplateResult } from 'lit';
import '@nigel/ui';

export function renderCategories(): TemplateResult {
  return html`
    <wc-empty-state
      icon="wc-icon-category"
      heading="Categories"
      message="Managing the chart of accounts arrives in task 31.16."
    ></wc-empty-state>
  `;
}

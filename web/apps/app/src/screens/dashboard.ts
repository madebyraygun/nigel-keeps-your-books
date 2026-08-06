import { html, type TemplateResult } from 'lit';
import '@nigel/ui';

export function renderDashboard(): TemplateResult {
  return html`
    <wc-empty-state
      icon="wc-icon-dashboard"
      heading="Dashboard"
      message="Year-to-date profit and loss, account balances, and the monthly income and expense chart arrive in task 31.11."
    ></wc-empty-state>
  `;
}

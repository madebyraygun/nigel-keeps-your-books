import { html, type TemplateResult } from 'lit';
import '@nigel/ui';

export function renderSettings(): TemplateResult {
  return html`
    <wc-empty-state
      icon="wc-icon-settings"
      heading="Settings"
      message="Business name, database password, and the auto-update toggle arrive in task 31.10."
    ></wc-empty-state>
  `;
}

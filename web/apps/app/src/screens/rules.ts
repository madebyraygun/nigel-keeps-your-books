import { html, type TemplateResult } from 'lit';
import '@nigel/ui';

export function renderRules(): TemplateResult {
  return html`
    <wc-empty-state
      icon="wc-icon-rule"
      heading="Rules"
      message="Viewing and editing categorization rules arrives in task 31.16."
    ></wc-empty-state>
  `;
}

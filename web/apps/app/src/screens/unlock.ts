import { html, type TemplateResult } from 'lit';
import '@nigel/ui';

export function renderUnlock(): TemplateResult {
  return html`
    <wc-empty-state
      icon="wc-icon-lock"
      heading="Database locked"
      message="This database is encrypted. The password form arrives in task 31.10; until then, unlock it from the terminal."
    ></wc-empty-state>
  `;
}

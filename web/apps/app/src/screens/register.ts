import { html, type TemplateResult } from 'lit';
import '@nigel/ui';

export function renderRegister(): TemplateResult {
  return html`
    <wc-empty-state
      icon="wc-icon-register"
      heading="Register"
      message="The searchable transaction register, with inline category and vendor editing, arrives in task 31.12."
    ></wc-empty-state>
  `;
}

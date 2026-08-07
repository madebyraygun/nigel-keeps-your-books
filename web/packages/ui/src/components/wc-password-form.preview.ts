import { html } from 'lit';
import './wc-password-form.js';
import type { Preview } from '../../preview/types.js';

const preview: Preview = {
  id: 'wc-password-form',
  title: 'Password Form',
  group: 'Forms',
  description:
    'Set, change, or remove the database password. The confirmation field never leaves the component.',
  layout: 'stack',
  states: [
    { name: 'set', render: () => html`<wc-password-form mode="set"></wc-password-form>` },
    {
      name: 'change',
      render: () => html`<wc-password-form mode="change"></wc-password-form>`,
    },
    {
      name: 'remove',
      render: () => html`<wc-password-form mode="remove"></wc-password-form>`,
    },
    {
      name: 'error',
      render: () =>
        html`<wc-password-form
          mode="change"
          error="Wrong password."
        ></wc-password-form>`,
    },
    {
      name: 'busy',
      render: () => html`<wc-password-form mode="set" busy></wc-password-form>`,
    },
  ],
};

export default preview;

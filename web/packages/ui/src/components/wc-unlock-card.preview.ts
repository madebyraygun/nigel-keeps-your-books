import { html } from 'lit';
import './wc-unlock-card.js';
import type { Preview } from '../../preview/types.js';

const preview: Preview = {
  id: 'wc-unlock-card',
  title: 'Unlock Card',
  group: 'Forms',
  description:
    'The gate in front of an encrypted database. The password never leaves the input and the submit event.',
  layout: 'stack',
  states: [
    {
      name: 'default',
      render: () => html`<wc-unlock-card heading="Raygun LLC"></wc-unlock-card>`,
    },
    {
      name: 'error',
      render: () =>
        html`<wc-unlock-card
          heading="Raygun LLC"
          error="Wrong password."
          attempts-remaining="2"
        ></wc-unlock-card>`,
    },
    {
      name: 'last-attempt',
      render: () =>
        html`<wc-unlock-card
          heading="Raygun LLC"
          error="Wrong password."
          attempts-remaining="0"
        ></wc-unlock-card>`,
    },
    {
      name: 'busy',
      render: () => html`<wc-unlock-card heading="Raygun LLC" busy></wc-unlock-card>`,
    },
    {
      name: 'backoff-countdown',
      render: () =>
        html`<wc-unlock-card
          heading="Raygun LLC"
          busy
          countdown-seconds="4"
        ></wc-unlock-card>`,
    },
  ],
};

export default preview;

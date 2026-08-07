import { html } from 'lit';
import './wc-period-nav.js';
import type { Preview } from '../../preview/types.js';

const preview: Preview = {
  id: 'wc-period-nav',
  title: 'Period nav',
  group: 'Navigation',
  layout: 'stack',
  description:
    'Granularity-driven period pager. The register allows an unfiltered "All"; report views offer year and month only, and a year-only report gets no granularity switch at all.',
  states: [
    {
      name: 'month',
      render: () => html`
        <wc-period-nav
          allow-all
          .period=${{ kind: 'month', year: 2025, month: 3 } as const}
        ></wc-period-nav>
      `,
    },
    {
      name: 'year',
      render: () => html`
        <wc-period-nav
          allow-all
          .period=${{ kind: 'year', year: 2025 } as const}
        ></wc-period-nav>
      `,
    },
    {
      name: 'all',
      render: () => html`
        <wc-period-nav allow-all .period=${{ kind: 'all' } as const}></wc-period-nav>
      `,
    },
    {
      name: 'report-view',
      render: () => html`
        <wc-period-nav .period=${{ kind: 'month', year: 2025, month: 6 } as const}></wc-period-nav>
      `,
    },
    {
      name: 'year-only',
      render: () => html`
        <wc-period-nav
          granularity="yearOnly"
          .period=${{ kind: 'year', year: 2024 } as const}
        ></wc-period-nav>
      `,
    },
    {
      name: 'disabled',
      render: () => html`
        <wc-period-nav
          allow-all
          disabled
          .period=${{ kind: 'year', year: 2025 } as const}
        ></wc-period-nav>
      `,
    },
  ],
};

export default preview;

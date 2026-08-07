import { css } from 'lit';

/**
 * What the app looks like on paper.
 *
 * A printed report is the artifact an accountant keeps, so the page has to be
 * the report and nothing else: no sidebar, no toolbars, no export buttons, no
 * screen-sized colour.
 *
 * The recolouring works by redefining the tokens at `:root` rather than by
 * restyling components. Custom properties inherit through shadow boundaries,
 * which is the one thing that reaches inside every `wc-*` element at once — a
 * print sheet cannot select into a shadow root, but it can change what the
 * shadow root reads. Everything else here is either `::part()` (the other way
 * through the boundary, which is why `wc-app-shell` exposes its furniture) or a
 * plain element selector, since slotted content stays in the document.
 */
export const printCss = css`
  @page {
    margin: 1.5cm;
  }

  @media print {
    :root {
      color-scheme: light;

      --wa-color-bg: #ffffff;
      --wa-color-surface: #ffffff;
      --wa-color-surface-alt: #ffffff;
      --wa-color-border: #999999;
      --wa-color-border-soft: #cccccc;
      --wa-color-text: #000000;
      --wa-color-muted: #333333;
      --wa-color-brand: #000000;
      --wa-color-brand-hover: #000000;
      --wa-color-on-brand: #ffffff;
      --wa-color-danger: #000000;
      --wa-color-success: #000000;
      --wa-color-warning: #000000;
      --wa-color-info: #000000;

      /* Income and expense stop being a colour pair on paper. wc-money always
         renders the sign, so the direction survives the loss of the hue. */
      --nc-color-income: #000000;
      --nc-color-expense: #000000;
      --nc-color-flagged: #000000;
      --nc-color-selected-bg: transparent;

      --nc-grad-brand: none;
      --nc-grad-brand-hover: none;

      --wa-shadow-s: none;
      --wa-shadow-m: none;
      --wa-shadow-l: none;
    }

    html,
    body {
      background: #ffffff;
      color: #000000;
    }

    /* The shell's own furniture, reached through the parts it exposes. */
    wc-app-shell::part(sidebar),
    wc-app-shell::part(header),
    wc-app-shell::part(banner) {
      display: none;
    }

    wc-app-shell::part(content) {
      display: block;
      padding: 0;
      overflow: visible;
    }

    /* Slotted and top-level chrome, which stays in the document. */
    wc-nav-sidebar,
    wc-toast,
    wc-export-links,
    wc-period-nav,
    wc-register-toolbar,
    wa-button,
    wa-select,
    [data-print='hide'] {
      display: none !important;
    }

    /* A report that runs over a page break keeps its column headings. */
    thead {
      display: table-header-group;
    }

    tr,
    wc-panel,
    wc-stat-card,
    wc-notice-bar {
      break-inside: avoid;
    }

    wc-panel,
    wc-stat-card {
      box-shadow: none;
    }

    a {
      text-decoration: none;
    }
  }
`;

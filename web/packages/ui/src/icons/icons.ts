import { customElement } from 'lit/decorators.js';
import { WcIconBase, svg } from './icon-base.js';

/**
 * Starter icon set: one per screen in the app's registry, plus the handful of
 * action glyphs the shell needs.
 *
 * The paths are plain geometry authored here rather than lifted from an icon
 * library, so there is no third-party license riding along in the binary.
 * Stroke width, caps and joins come from WcIconBase; each icon supplies only
 * its geometry on a 24x24 grid.
 */

@customElement('wc-icon-dashboard')
export class WcIconDashboard extends WcIconBase {
  protected renderIcon() {
    return svg`<path d="M4 4h6v6H4zM14 4h6v6h-6zM4 14h6v6H4zM14 14h6v6h-6z"/>`;
  }
}

@customElement('wc-icon-register')
export class WcIconRegister extends WcIconBase {
  protected renderIcon() {
    return svg`<path d="M3 5h18v14H3zM3 10h18M9 10v9"/>`;
  }
}

@customElement('wc-icon-review')
export class WcIconReview extends WcIconBase {
  protected renderIcon() {
    return svg`<path d="M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18z"/><path d="M8 12l2.5 2.5L16 9"/>`;
  }
}

@customElement('wc-icon-import')
export class WcIconImport extends WcIconBase {
  protected renderIcon() {
    return svg`<path d="M12 3v12M8 11l4 4 4-4M4 19h16"/>`;
  }
}

@customElement('wc-icon-report')
export class WcIconReport extends WcIconBase {
  protected renderIcon() {
    return svg`<path d="M3 20h18M6 20V11M12 20V4M18 20v-6"/>`;
  }
}

@customElement('wc-icon-account')
export class WcIconAccount extends WcIconBase {
  protected renderIcon() {
    return svg`<path d="M3 7h18v12H3zM3 7l3-3h12l3 3M16 13h2"/>`;
  }
}

@customElement('wc-icon-category')
export class WcIconCategory extends WcIconBase {
  protected renderIcon() {
    return svg`<path d="M3 12l9-9h8v8l-9 9z"/><path d="M16.5 7.5h.01"/>`;
  }
}

@customElement('wc-icon-rule')
export class WcIconRule extends WcIconBase {
  protected renderIcon() {
    return svg`<path d="M3 4h18l-7 8v7l-4 2v-9z"/>`;
  }
}

@customElement('wc-icon-reconcile')
export class WcIconReconcile extends WcIconBase {
  protected renderIcon() {
    return svg`<path d="M4 8h13l-3-3M20 16H7l3 3"/>`;
  }
}

@customElement('wc-icon-undo')
export class WcIconUndo extends WcIconBase {
  protected renderIcon() {
    return svg`<path d="M4 10h10a5 5 0 0 1 0 10H8M4 10l4-4M4 10l4 4"/>`;
  }
}

@customElement('wc-icon-settings')
export class WcIconSettings extends WcIconBase {
  protected renderIcon() {
    return svg`<path d="M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6z"/><path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9L7 7M17 17l2.1 2.1M19.1 4.9L17 7M7 17l-2.1 2.1"/>`;
  }
}

@customElement('wc-icon-lock')
export class WcIconLock extends WcIconBase {
  protected renderIcon() {
    return svg`<path d="M6 11h12v9H6zM9 11V7a3 3 0 0 1 6 0v4"/>`;
  }
}

@customElement('wc-icon-search')
export class WcIconSearch extends WcIconBase {
  protected renderIcon() {
    return svg`<path d="M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14z"/><path d="M16 16l4 4"/>`;
  }
}

@customElement('wc-icon-close')
export class WcIconClose extends WcIconBase {
  protected renderIcon() {
    return svg`<path d="M6 6l12 12M18 6L6 18"/>`;
  }
}

@customElement('wc-icon-check')
export class WcIconCheck extends WcIconBase {
  protected renderIcon() {
    return svg`<path d="M4 12l5 5L20 6"/>`;
  }
}

@customElement('wc-icon-flag')
export class WcIconFlag extends WcIconBase {
  protected renderIcon() {
    return svg`<path d="M5 21V4h13l-3 4 3 4H5"/>`;
  }
}

@customElement('wc-icon-chevron-left')
export class WcIconChevronLeft extends WcIconBase {
  protected renderIcon() {
    return svg`<path d="M15 6l-6 6 6 6"/>`;
  }
}

@customElement('wc-icon-chevron-right')
export class WcIconChevronRight extends WcIconBase {
  protected renderIcon() {
    return svg`<path d="M9 6l6 6-6 6"/>`;
  }
}

/** Every tag this module registers, for previews and the registry's icon check. */
export const ICON_TAGS = [
  'wc-icon-dashboard',
  'wc-icon-register',
  'wc-icon-review',
  'wc-icon-import',
  'wc-icon-report',
  'wc-icon-account',
  'wc-icon-category',
  'wc-icon-rule',
  'wc-icon-reconcile',
  'wc-icon-undo',
  'wc-icon-settings',
  'wc-icon-lock',
  'wc-icon-search',
  'wc-icon-close',
  'wc-icon-check',
  'wc-icon-flag',
  'wc-icon-chevron-left',
  'wc-icon-chevron-right',
] as const;

export type IconTag = (typeof ICON_TAGS)[number];

declare global {
  interface HTMLElementTagNameMap {
    'wc-icon-dashboard': WcIconDashboard;
    'wc-icon-register': WcIconRegister;
    'wc-icon-review': WcIconReview;
    'wc-icon-import': WcIconImport;
    'wc-icon-report': WcIconReport;
    'wc-icon-account': WcIconAccount;
    'wc-icon-category': WcIconCategory;
    'wc-icon-rule': WcIconRule;
    'wc-icon-reconcile': WcIconReconcile;
    'wc-icon-undo': WcIconUndo;
    'wc-icon-settings': WcIconSettings;
    'wc-icon-lock': WcIconLock;
    'wc-icon-search': WcIconSearch;
    'wc-icon-close': WcIconClose;
    'wc-icon-check': WcIconCheck;
    'wc-icon-flag': WcIconFlag;
    'wc-icon-chevron-left': WcIconChevronLeft;
    'wc-icon-chevron-right': WcIconChevronRight;
  }
}

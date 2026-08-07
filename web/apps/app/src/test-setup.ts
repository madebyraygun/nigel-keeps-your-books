// Screens mount @nigel/ui components, which wrap Web Awesome primitives, so
// the app suite needs the same jsdom gaps filled as the component library.
// Guarded on `window` because most of this package's tests run under node.
if (typeof window !== 'undefined') {
  const EI = (globalThis as Record<string, unknown>)['ElementInternals'] as
    | { prototype: Record<string, unknown> }
    | undefined;
  if (EI?.prototype) {
    const proto = EI.prototype;
    const noop = () => {};
    for (const method of [
      'setValidity',
      'setFormValue',
      'setCustomValidity',
      'reportValidity',
      'checkValidity',
    ]) {
      if (typeof proto[method] !== 'function') proto[method] = noop;
    }

    // Web Awesome's form controls read `internals.validity` during their first
    // update. jsdom does not define it, and the resulting throw escapes as an
    // unhandled rejection rather than a failing assertion.
    if (!('validity' in proto)) {
      Object.defineProperty(proto, 'validity', {
        get() {
          return {
            valid: true,
            valueMissing: false,
            typeMismatch: false,
            patternMismatch: false,
            tooLong: false,
            tooShort: false,
            rangeUnderflow: false,
            rangeOverflow: false,
            stepMismatch: false,
            badInput: false,
            customError: false,
          };
        },
        configurable: true,
      });
    }
    if (!('validationMessage' in proto)) {
      Object.defineProperty(proto, 'validationMessage', {
        get() {
          return '';
        },
        configurable: true,
      });
    }
  }

  if (window.HTMLDialogElement) {
    const dialogProto = window.HTMLDialogElement.prototype;
    if (typeof dialogProto.showModal !== 'function') {
      dialogProto.showModal = function (this: HTMLDialogElement) {
        this.open = true;
      };
    }
    if (typeof dialogProto.close !== 'function') {
      dialogProto.close = function (this: HTMLDialogElement) {
        this.open = false;
      };
    }
  }

  if (window.Element) {
    const proto = window.Element.prototype as Element & {
      getAnimations?: () => Animation[];
    };
    if (typeof proto.getAnimations !== 'function') proto.getAnimations = () => [];
  }

  if (window.HTMLElement) {
    const proto = window.HTMLElement.prototype as HTMLElement & {
      showPopover?: () => void;
      hidePopover?: () => void;
    };
    if (typeof proto.showPopover !== 'function') proto.showPopover = function () {};
    if (typeof proto.hidePopover !== 'function') proto.hidePopover = function () {};
  }
}

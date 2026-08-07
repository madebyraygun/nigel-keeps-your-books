import axe from 'axe-core';

// Unit tests render isolated components straight into document.body, so the
// "region" rule (all content must sit inside a landmark) does not apply — the
// page scaffolding it wants is the app's job, not a component's.
axe.configure({
  rules: [{ id: 'region', enabled: false }],
});

// jsdom implements only part of ElementInternals. Web Awesome form controls
// call the missing members during their first update and would otherwise throw
// unhandled rejections in every test that mounts one.
{
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
      if (typeof proto[method] !== 'function') {
        proto[method] = noop;
      }
    }
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
}

// jsdom's HTMLDialogElement has no showModal()/show()/close(). <wa-dialog>
// calls showModal() internally, so wc-confirm is untestable without these.
if (typeof window !== 'undefined' && window.HTMLDialogElement) {
  const dialogProto = window.HTMLDialogElement.prototype;
  if (typeof dialogProto.showModal !== 'function') {
    dialogProto.showModal = function (this: HTMLDialogElement) {
      this.open = true;
    };
  }
  if (typeof dialogProto.show !== 'function') {
    dialogProto.show = function (this: HTMLDialogElement) {
      this.open = true;
    };
  }
  if (typeof dialogProto.close !== 'function') {
    dialogProto.close = function (this: HTMLDialogElement) {
      this.open = false;
    };
  }
}

// jsdom implements no Web Animations API. Web Awesome awaits getAnimations()
// on every dialog show/hide from a requestAnimationFrame callback, where a
// throw escapes as an uncaught exception rather than a rejected test. An empty
// list reads as "nothing is animating", which is true here.
if (typeof window !== 'undefined' && window.Element) {
  const proto = window.Element.prototype as Element & {
    getAnimations?: () => Animation[];
  };
  if (typeof proto.getAnimations !== 'function') {
    proto.getAnimations = () => [];
  }
}

// The toast region promotes itself into the top layer so it paints above
// wa-dialog's native modal. jsdom has no Popover API; without these the
// component's guarded calls simply no-op, which is the intended fallback.
if (typeof window !== 'undefined' && window.HTMLElement) {
  const proto = window.HTMLElement.prototype as HTMLElement & {
    showPopover?: () => void;
    hidePopover?: () => void;
  };
  if (typeof proto.showPopover !== 'function') {
    proto.showPopover = function () {};
  }
  if (typeof proto.hidePopover !== 'function') {
    proto.hidePopover = function () {};
  }
}

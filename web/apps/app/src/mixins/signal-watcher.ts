/**
 * The single seam onto @lit-labs/signals.
 *
 * Everything in the app imports signal primitives from here, so swapping the
 * reactivity library later is one file rather than a search across every
 * store and component.
 */
export { SignalWatcher, signal, computed, Signal } from '@lit-labs/signals';

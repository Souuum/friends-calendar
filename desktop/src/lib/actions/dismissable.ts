/**
 * Gives a modal every way out a person might reasonably try.
 *
 * ## Why this exists
 *
 * The modals here shipped with exactly one escape hatch: a `×` glyph with
 * no padding, measured at **14.7 × 32px** - a third of the 44px tap target
 * the bottom tab bar is held to - sitting at the top of a form long enough
 * that `Cancel` was 1005px down a scroll container. On a phone that is
 * effectively no way out at all, so people fall back to the browser's back
 * gesture, which navigates off the page instead of closing the modal.
 *
 * Three routes out, all of them expected by somebody:
 *
 * - **Escape**, for a keyboard.
 * - **Tapping the backdrop**, which is the near-universal convention.
 * - **The back gesture**, which is how a phone user closes anything.
 *
 * ## The history entry
 *
 * Back only closes the modal if the modal *is* a history entry, so opening
 * one pushes a state and closing it pops that state back off. Two details
 * matter:
 *
 * 1. **SvelteKit's own state is preserved** (`...history.state`). The
 *    router keys navigation off an index it stores there; replacing the
 *    state object wholesale would strip it and confuse the next real
 *    navigation.
 * 2. **The entry is cleaned up on close.** Dismissing via a button pops it
 *    with `history.back()`, so a modal opened and closed doesn't leave a
 *    dead entry that makes the user press back twice to leave the page.
 *    `poppedByBrowser` distinguishes the two, so we never call `back()`
 *    for an entry the browser has already removed - that would navigate
 *    away, which is the bug being fixed.
 */

/** Marks our own history entries so a popstate can be attributed. */
const MODAL_STATE_KEY = '__modal';

export function dismissable(node: HTMLElement, onDismiss: () => void) {
  let dismiss = onDismiss;

  // True while the entry we pushed is still on the stack. Goes false the
  // moment the browser pops it, so teardown knows not to pop it again.
  let ourEntryIsLive = false;

  const browser = typeof window !== 'undefined';

  function onKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.stopPropagation();
      dismiss();
    }
  }

  // Only the backdrop itself - a click that started inside the dialog must
  // not close it, or dragging to select text and releasing outside would.
  function onPointerDown(event: PointerEvent) {
    if (event.target === node) {
      const onUp = (up: PointerEvent) => {
        if (up.target === node) dismiss();
      };
      node.addEventListener('pointerup', onUp, { once: true });
    }
  }

  function onPopState() {
    ourEntryIsLive = false;
    dismiss();
  }

  if (browser) {
    try {
      history.pushState({ ...history.state, [MODAL_STATE_KEY]: true }, '');
      ourEntryIsLive = true;
    } catch {
      // Some embedded webviews refuse pushState. The modal still works;
      // only the back gesture falls back to normal navigation.
    }
    window.addEventListener('keydown', onKeydown);
    window.addEventListener('popstate', onPopState);
    node.addEventListener('pointerdown', onPointerDown);
  }

  return {
    update(next: () => void) {
      dismiss = next;
    },
    destroy() {
      if (!browser) return;
      window.removeEventListener('keydown', onKeydown);
      window.removeEventListener('popstate', onPopState);
      node.removeEventListener('pointerdown', onPointerDown);
      // Closed by a control rather than by the browser: take our entry back
      // off, so leaving the page still takes one press of back.
      if (ourEntryIsLive) history.back();
    }
  };
}

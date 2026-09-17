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
 * ## ⚠️ One history entry per *session*, not per modal
 *
 * The obvious design - each instance pushes its own entry on mount and pops
 * it on unmount - cannot survive one modal opening another, because
 * `history.back()` is **asynchronous**: it queues a pop that lands a task
 * later, by which time the replacement modal has pushed an entry of its
 * own. The invite sheet on `/friends/[id]` opening the create-event form
 * produced exactly that:
 *
 *     pushState   sheet opens
 *     back()      sheet destroyed, pop queued
 *     pushState   create-event form opens, pushes its own entry
 *     popstate    the queued pop lands and eats the FORM's entry
 *
 * The form appeared for one frame and vanished - reported as "the create
 * event modal never appear". Suppressing that stray popstate is not enough
 * on its own: the entry is still gone, so the back gesture would navigate
 * off the page again, which is the bug this whole module exists to fix.
 *
 * So the *stack* owns the entry, not the instance. One entry is pushed when
 * the first modal opens and popped once the last one closes, and a handoff
 * between two modals touches history not at all - the teardown pop is
 * deferred by a task, which is long enough for the replacement to cancel
 * it. `selfInitiatedPops` still guards the pop we do eventually make, since
 * a popstate says nothing about which entry it removed.
 */

/** Marks our own history entry so a popstate can be attributed. */
const MODAL_STATE_KEY = '__modal';

type Instance = {
  dismiss: () => void;
};

/** Mounted instances, oldest first. The last one is the topmost modal. */
const stack: Instance[] = [];

/** Whether the entry we push for an open session is currently on the stack. */
let weOwnAnEntry = false;

/**
 * Pops *we* queued while closing a session. A popstate arriving while this
 * is non-zero is our own cleanup, not the user pressing back.
 */
let selfInitiatedPops = 0;

/** Set while a teardown pop is waiting out the task that lets a handoff
 *  cancel it. */
let pendingRelease: ReturnType<typeof setTimeout> | null = null;

let sharedListenersInstalled = false;

function pushOurEntry() {
  try {
    history.pushState({ ...history.state, [MODAL_STATE_KEY]: true }, '');
    weOwnAnEntry = true;
  } catch {
    // Some embedded webviews refuse pushState. The modal still works; only
    // the back gesture falls back to normal navigation.
  }
}

/** Takes our entry back off, so leaving the page still takes one press of
 *  back. Deferred, so a modal replacing another can cancel it. */
function scheduleRelease() {
  if (pendingRelease !== null) return;
  pendingRelease = setTimeout(() => {
    pendingRelease = null;
    if (stack.length > 0 || !weOwnAnEntry) return;
    weOwnAnEntry = false;
    selfInitiatedPops += 1;
    history.back();
  }, 0);
}

function onPopState() {
  if (selfInitiatedPops > 0) {
    selfInitiatedPops -= 1;
    return;
  }
  const top = stack[stack.length - 1];
  if (!top) return;
  // The browser has taken our entry; don't try to pop it again.
  weOwnAnEntry = false;
  top.dismiss();
  // Modals left underneath still need an entry to absorb the next press,
  // or back would navigate off the page with one still open.
  if (stack.length > 0 && !weOwnAnEntry) pushOurEntry();
}

function onKeydown(event: KeyboardEvent) {
  if (event.key !== 'Escape') return;
  const top = stack[stack.length - 1];
  if (!top) return;
  event.stopPropagation();
  top.dismiss();
}

function installSharedListeners() {
  if (sharedListenersInstalled) return;
  window.addEventListener('keydown', onKeydown);
  window.addEventListener('popstate', onPopState);
  sharedListenersInstalled = true;
}

export function dismissable(node: HTMLElement, onDismiss: () => void) {
  const browser = typeof window !== 'undefined';
  const instance: Instance = { dismiss: onDismiss };

  // Only the backdrop itself - a click that started inside the dialog must
  // not close it, or dragging to select text and releasing outside would.
  function onPointerDown(event: PointerEvent) {
    if (event.target !== node) return;
    const onUp = (up: PointerEvent) => {
      if (up.target === node) instance.dismiss();
    };
    node.addEventListener('pointerup', onUp, { once: true });
  }

  if (browser) {
    // A modal replacing another one: keep the entry the session already has
    // rather than releasing and re-pushing it.
    if (pendingRelease !== null) {
      clearTimeout(pendingRelease);
      pendingRelease = null;
    }
    stack.push(instance);
    if (!weOwnAnEntry) pushOurEntry();
    installSharedListeners();
    node.addEventListener('pointerdown', onPointerDown);
  }

  return {
    update(next: () => void) {
      instance.dismiss = next;
    },
    destroy() {
      if (!browser) return;
      node.removeEventListener('pointerdown', onPointerDown);
      const at = stack.indexOf(instance);
      if (at !== -1) stack.splice(at, 1);
      if (stack.length === 0) scheduleRelease();
    }
  };
}

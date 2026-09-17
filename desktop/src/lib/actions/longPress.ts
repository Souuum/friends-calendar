/**
 * "Open this" without a click: a long press on touch, a double-click with a
 * mouse.
 *
 * ## Why an action, and why both gestures
 *
 * `Friends Calendar Mobile.dc.html`'s calendar screen says *"long-press an
 * empty day to start an event there"*. Long press has **no native event**,
 * so it has to be assembled from pointer events - and nobody holds a mouse
 * button down on a calendar, so the desktop equivalent is a double-click.
 * One action offers both so a call site doesn't have to know which input
 * the person is using.
 *
 * ## The part that breaks if you skip it
 *
 * ⚠️ **Cancel on movement, not just on pointerup.** A finger that presses
 * and then drags is *scrolling*, and without a movement threshold every
 * scroll that starts on a day cell fires the long press. `pointercancel`
 * covers the browser taking over the gesture, but it arrives too late to
 * be the only guard.
 */

/** Milliseconds a press must be held. ~half a second is the usual feel. */
const HOLD_MS = 500;

/** Movement past this many pixels means the person is scrolling, not pressing. */
const MOVE_TOLERANCE_PX = 10;

export function longPress(node: HTMLElement, onTrigger: () => void) {
  let trigger = onTrigger;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let origin: { x: number; y: number } | null = null;

  function cancel() {
    clearTimeout(timer);
    timer = undefined;
    origin = null;
  }

  function onPointerDown(event: PointerEvent) {
    // Mouse users get the double-click instead; starting a hold timer for
    // them means a click-and-think fires it.
    if (event.pointerType === 'mouse') return;
    origin = { x: event.clientX, y: event.clientY };
    timer = setTimeout(() => {
      cancel();
      trigger();
    }, HOLD_MS);
  }

  function onPointerMove(event: PointerEvent) {
    if (!origin) return;
    const moved =
      Math.abs(event.clientX - origin.x) > MOVE_TOLERANCE_PX ||
      Math.abs(event.clientY - origin.y) > MOVE_TOLERANCE_PX;
    if (moved) cancel();
  }

  function onDoubleClick() {
    trigger();
  }

  node.addEventListener('pointerdown', onPointerDown);
  node.addEventListener('pointermove', onPointerMove);
  node.addEventListener('pointerup', cancel);
  node.addEventListener('pointercancel', cancel);
  node.addEventListener('pointerleave', cancel);
  node.addEventListener('dblclick', onDoubleClick);

  return {
    update(next: () => void) {
      trigger = next;
    },
    destroy() {
      cancel();
      node.removeEventListener('pointerdown', onPointerDown);
      node.removeEventListener('pointermove', onPointerMove);
      node.removeEventListener('pointerup', cancel);
      node.removeEventListener('pointercancel', cancel);
      node.removeEventListener('pointerleave', cancel);
      node.removeEventListener('dblclick', onDoubleClick);
    }
  };
}

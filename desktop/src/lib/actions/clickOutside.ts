export function clickOutside(node: HTMLElement, callback: () => void) {
  function handleClick(event: MouseEvent) {
    if (!node.contains(event.target as Node)) {
      callback();
    }
  }

  document.addEventListener('click', handleClick);

  return {
    destroy() {
      // No `true` here: the listener was added in the bubble phase, and a
      // mismatched capture flag means removeEventListener silently matches
      // nothing - so every use of this action leaked its listener.
      document.removeEventListener('click', handleClick);
    }
  };
}

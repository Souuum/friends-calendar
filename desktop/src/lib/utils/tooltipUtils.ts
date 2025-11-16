export interface TooltipPosition {
  x: number;
  y: number;
}

export interface TooltipConfig {
  tooltipWidth?: number;
  tooltipHeight?: number;
  offset?: number;
  screenPadding?: number;
}

const DEFAULT_CONFIG: Required<TooltipConfig> = {
  tooltipWidth: 384, // w-96
  tooltipHeight: 500,
  offset: 10,
  screenPadding: 20
};

/**
 * Calculates the optimal position for a tooltip relative to a target element
 * Ensures the tooltip stays within viewport boundaries
 */
export function calculateTooltipPosition(
  targetRect: DOMRect,
  config: TooltipConfig = {}
): TooltipPosition {
  const { tooltipWidth, tooltipHeight, offset, screenPadding } = { ...DEFAULT_CONFIG, ...config };

  const viewportWidth = window.innerWidth;
  const viewportHeight = window.innerHeight;
  const scrollY = window.scrollY;

  let x = 0;
  let y = targetRect.top + scrollY;

  const rightPosition = targetRect.right + offset;
  const leftPosition = targetRect.left - tooltipWidth - offset;

  if (rightPosition + tooltipWidth + screenPadding <= viewportWidth) {
    x = rightPosition;
  } else if (leftPosition >= screenPadding) {
    x = leftPosition;
  } else {
    x = Math.max(screenPadding, (viewportWidth - tooltipWidth) / 2);
  }

  if (y + tooltipHeight > viewportHeight + scrollY - screenPadding) {
    if (targetRect.top > viewportHeight - targetRect.bottom) {
      y = targetRect.top + scrollY - tooltipHeight - offset;
    } else {
      y = viewportHeight + scrollY - tooltipHeight - screenPadding;
    }
  }

  y = Math.max(scrollY + screenPadding, y);

  return { x, y };
}

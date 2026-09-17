<script lang="ts">
  export let hour: number;
  export let align: 'left' | 'right' = 'right';
  /**
   * Drops the minutes and pads less: "9 AM" instead of "09:00 AM". The
   * mobile grids give the time gutter 56px, where the full form wraps.
   */
  export let compact = false;

  function formatHour(h: number): string {
    if (h === 0) return '12:00 AM';
    if (h === 12) return '12:00 PM';
    return h < 12
      ? `${h.toString().padStart(2, '0')}:00 AM`
      : `${(h - 12).toString().padStart(2, '0')}:00 PM`;
  }

  function formatCompact(h: number): string {
    if (h === 0) return '12 AM';
    if (h === 12) return '12 PM';
    return h < 12 ? `${h} AM` : `${h - 12} PM`;
  }
</script>

<div class="text-xs text-gray-500 pr-2 text-{align}">
  {compact ? formatCompact(hour) : formatHour(hour)}
</div>

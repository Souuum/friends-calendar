<script lang="ts">
  export let hour: number;
  export let align: 'left' | 'right' = 'right';

  function formatHour(h: number): string {
    if (h === 0) return '12:00 AM';
    if (h === 12) return '12:00 PM';
    return h < 12
      ? `${h.toString().padStart(2, '0')}:00 AM`
      : `${(h - 12).toString().padStart(2, '0')}:00 PM`;
  }
</script>

<div class="text-xs text-gray-500 pr-2 text-{align}">
  {formatHour(hour)}
</div>

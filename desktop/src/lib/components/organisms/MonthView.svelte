<script lang="ts">
  import type { EventWithParticipants } from '$lib/types';
  import WeekdayHeader from '$lib/components/atoms/WeekdayHeader.svelte';
  import CalendarDay from '$lib/components/atoms/CalendarDay.svelte';
  import EventList from '$lib/components/molecules/EventList.svelte';
  import { createEventDispatcher } from 'svelte';
  import { calculateTooltipPosition } from '$lib/utils/tooltipUtils';
  import { longPress } from '$lib/actions/longPress';

  export let monthGrid: Date[];
  export let currentMonth: Date;
  export let eventsForDay: (day: Date) => EventWithParticipants[];
  export let onEventClick: ((event: EventWithParticipants) => void) | undefined = undefined;
  /** The day currently filtering the list below the grid, if any. */
  export let selectedDay: Date | null = null;
  export let onDayClick: ((day: Date) => void) | undefined = undefined;
  /** Long press (touch) or double-click (mouse): start an event on that day. */
  export let onDayLongPress: ((day: Date) => void) | undefined = undefined;

  function isSameDay(a: Date | null, b: Date): boolean {
    return (
      a !== null &&
      a.getFullYear() === b.getFullYear() &&
      a.getMonth() === b.getMonth() &&
      a.getDate() === b.getDate()
    );
  }

  const dispatch = createEventDispatcher();

  let hoverTimer: ReturnType<typeof setTimeout> | null = null;
  let hideTimer: ReturnType<typeof setTimeout> | null = null;
  let hoveredDay: Date | null = null;
  let isTooltipHovered = false;

  function isToday(date: Date): boolean {
    const today = new Date();
    return (
      date.getFullYear() === today.getFullYear() &&
      date.getMonth() === today.getMonth() &&
      date.getDate() === today.getDate()
    );
  }

  function isCurrentMonth(date: Date): boolean {
    return date.getMonth() === currentMonth.getMonth();
  }

  // The mockup dims days that have already been and gone (0.72) separately
  // from days outside the month (0.45), so "past" and "not this month" stay
  // distinguishable.
  function isPast(date: Date): boolean {
    const startOfToday = new Date();
    startOfToday.setHours(0, 0, 0, 0);
    return date < startOfToday;
  }

  function handleMouseEnter(day: Date, e: MouseEvent) {
    const dayEvents = eventsForDay(day);
    if (dayEvents.length === 0) return;

    const target = e.currentTarget as HTMLElement | null;
    if (!target) return;

    if (hideTimer) {
      clearTimeout(hideTimer);
      hideTimer = null;
    }

    if (hoverTimer) {
      clearTimeout(hoverTimer);
    }

    hoverTimer = setTimeout(() => {
      hoveredDay = day;

      const rect = target.getBoundingClientRect();
      const tooltipPosition = calculateTooltipPosition(rect);

      dispatch('showTooltip', { day, events: dayEvents, position: tooltipPosition });
    }, 500);
  }

  function handleMouseLeave() {
    if (hoverTimer) {
      clearTimeout(hoverTimer);
      hoverTimer = null;
    }

    hideTimer = setTimeout(() => {
      if (!isTooltipHovered) {
        hoveredDay = null;
        dispatch('hideTooltip');
      }
    }, 150);
  }

  function handleTooltipMouseEnter() {
    isTooltipHovered = true;
    if (hideTimer) {
      clearTimeout(hideTimer);
      hideTimer = null;
    }
  }

  function handleTooltipMouseLeave() {
    isTooltipHovered = false;
    hideTimer = setTimeout(() => {
      hoveredDay = null;
      dispatch('hideTooltip');
    }, 150);
  }

  export { handleTooltipMouseEnter, handleTooltipMouseLeave };
</script>

<div>
  <WeekdayHeader />

  <!-- gap-px over a `line` background is how the mockup draws its gridlines:
       one hairline between cells rather than a border on each, which would
       double up and leave the outer edge twice as heavy. The rounded outer
       border plus overflow-hidden is what gives the grid its card edge - the
       app had neither, so the month view bled into the page. -->
  <div class="grid grid-cols-7 gap-px overflow-hidden rounded-xl border border-line bg-line">
    {#each monthGrid as day}
      {@const dayEvents = eventsForDay(day)}
      {@const outside = !isCurrentMonth(day)}
      <!-- These cells carried `role="button"`, `tabindex="0"` and a pointer
           cursor with **no handler at all** - 35 promises per screen that
           the component didn't keep, and worse than a plain div because a
           screen reader announced them as buttons. Now they select. -->
      <div
        role="button"
        tabindex="0"
        aria-pressed={isSameDay(selectedDay, day)}
        class="flex min-h-[126px] cursor-pointer flex-col px-[9px] pb-2.5 pt-[9px] text-left transition-colors
          {isSameDay(selectedDay, day) ? 'bg-tint' : 'bg-surface'}
          {outside ? 'opacity-45' : isPast(day) ? 'opacity-[0.72]' : ''}"
        on:click={() => onDayClick?.(day)}
        on:keydown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            onDayClick?.(day);
          }
        }}
        use:longPress={() => onDayLongPress?.(day)}
        on:mouseenter={(e) => handleMouseEnter(day, e)}
        on:mouseleave={handleMouseLeave}
      >
        <div class="mb-[5px] flex items-center gap-1.5">
          <CalendarDay date={day} isToday={isToday(day)} isCurrentMonth={!outside} />
        </div>

        <EventList
          events={dayEvents}
          variant="compact"
          maxVisible={3}
          showMore={true}
          {onEventClick}
        />
      </div>
    {/each}
  </div>
</div>

<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import type { EventWithParticipants, FriendInfo } from '$lib/types';
  import CalendarHeader from '$lib/components/molecules/CalendarHeader.svelte';
  import MonthView from '$lib/components/organisms/MonthView.svelte';
  import WeekView from '$lib/components/organisms/WeekView.svelte';
  import DayView from '$lib/components/organisms/DayView.svelte';
  import EventTooltip from '$lib/components/molecules/EventTooltip.svelte';
  import EventPeekPanel from '$lib/components/organisms/EventPeekPanel.svelte';
  import CreateEventModal from '$lib/components/CreateEventModal.svelte';
  import { dateUtils } from '$lib/utils/dateUtils';
  import { createEventDispatcher } from 'svelte';

  export let events: EventWithParticipants[] = [];

  const dispatch = createEventDispatcher();

  type ViewType = 'month' | 'week' | 'day';
  let view: ViewType = 'month';
  let currentDate = new Date();

  // Tooltip state (for month view)
  let tooltipVisible = false;
  let tooltipEvents: EventWithParticipants[] = [];
  let tooltipPosition = { x: 0, y: 0 };
  let monthViewRef: MonthView;

  // Peek panel state (month/week/day - replaces the old modal-on-click,
  // see .claude/skills/mockup-calendar-redesign/SKILL.md).
  let selectedEvent: EventWithParticipants | null = null;

  // Create-event modal, lifted here (from CalendarHeader) so the
  // Free-tonight bar's "Propose a time" button opens the same instance as
  // the header's "+ New Event" button.
  //
  // `editingEvent` doubles as the mode switch rather than being a second
  // boolean: null means the modal is creating, an event means it's editing
  // that one, so the two can't disagree.
  let showCreateModal = false;
  let editingEvent: EventWithParticipants | null = null;

  function openCreateModal() {
    editingEvent = null;
    showCreateModal = true;
  }

  function openEditModal(event: EventWithParticipants) {
    editingEvent = event;
    showCreateModal = true;
  }

  function closeModal() {
    showCreateModal = false;
    editingEvent = null;
  }

  function handleSaved() {
    closeModal();
    dispatch('refresh');
  }

  function handleDeleted() {
    // The panel's selection is now a dangling id - clear it before the
    // parent refetches, or the panel renders a deleted event until the
    // new list arrives.
    selectedEvent = null;
    dispatch('refresh');
  }

  // Free-tonight bar
  let freeFriends: FriendInfo[] = [];
  let freeTonightError = '';

  type FilterKey = 'all' | 'going' | 'awaiting' | 'mine';
  let activeFilter: FilterKey = 'all';
  // Labels match the mockup's calFilters copy exactly - "Awaiting my
  // answer" and "Created by me", not paraphrased versions.
  const filters: { key: FilterKey; label: string }[] = [
    { key: 'all', label: 'All events' },
    { key: 'going', label: 'Going' },
    { key: 'awaiting', label: 'Awaiting my answer' },
    { key: 'mine', label: 'Created by me' }
  ];

  async function loadFreeTonight() {
    try {
      freeTonightError = '';
      const [freeIds, friends] = await Promise.all([api.getFreeFriendsNow(), api.getFriends()]);
      const freeIdSet = new Set(freeIds);
      freeFriends = friends.filter((f) => freeIdSet.has(f.user_id));
    } catch (err) {
      // Best-effort, like /friends's own free-now pill - a failure here
      // shouldn't block the calendar itself from rendering.
      freeTonightError = err instanceof Error ? err.message : 'Failed to load availability';
    }
  }

  onMount(loadFreeTonight);

  function handleShowTooltip(event: CustomEvent) {
    tooltipEvents = event.detail.events;
    tooltipPosition = event.detail.position;
    tooltipVisible = true;
  }

  function handleHideTooltip() {
    tooltipVisible = false;
  }

  function handleTooltipMouseEnter() {
    if (monthViewRef) {
      monthViewRef.handleTooltipMouseEnter();
    }
  }

  function handleTooltipMouseLeave() {
    if (monthViewRef) {
      monthViewRef.handleTooltipMouseLeave();
    }
  }

  function selectEvent(event: EventWithParticipants) {
    selectedEvent = event;
  }

  function handleRefresh() {
    dispatch('refresh');
    loadFreeTonight();
  }

  function prev(): void {
    if (view === 'month') {
      currentDate = new Date(currentDate.getFullYear(), currentDate.getMonth() - 1, 1);
    } else if (view === 'week') {
      const newDate = new Date(currentDate);
      newDate.setDate(newDate.getDate() - 7);
      currentDate = newDate;
    } else {
      const newDate = new Date(currentDate);
      newDate.setDate(newDate.getDate() - 1);
      currentDate = newDate;
    }
  }

  function next(): void {
    if (view === 'month') {
      currentDate = new Date(currentDate.getFullYear(), currentDate.getMonth() + 1, 1);
    } else if (view === 'week') {
      const newDate = new Date(currentDate);
      newDate.setDate(newDate.getDate() + 7);
      currentDate = newDate;
    } else {
      const newDate = new Date(currentDate);
      newDate.setDate(newDate.getDate() + 1);
      currentDate = newDate;
    }
  }

  function goToToday(): void {
    currentDate = new Date();
  }

  function handleViewChange(e: CustomEvent<ViewType>): void {
    view = e.detail;
  }

  function matchesFilter(event: EventWithParticipants, filter: FilterKey): boolean {
    switch (filter) {
      case 'going':
        return event.my_status === 'accepted';
      case 'awaiting':
        // `is_participant` matters here: without it this sweeps up every
        // public event in the server, since a non-participant has no
        // my_status either. You can only owe an answer if you were asked.
        return event.is_participant && (!event.my_status || event.my_status === 'pending');
      case 'mine':
        return event.is_creator;
      default:
        return true;
    }
  }

  // `activeFilter` has to appear directly in this expression (not just
  // inside matchesFilter's body) - Svelte's reactive-statement dependency
  // tracking is static, based on identifiers referenced in the `$:` line
  // itself, not on what a called function transitively reads.
  $: filteredEvents = events.filter((event) => matchesFilter(event, activeFilter));

  // Declared reactively (not a plain function) so its reference changes
  // whenever `filteredEvents` does - MonthView/WeekView/DayView receive
  // this as a prop, and a child only re-invokes a function prop when the
  // prop's own reference changes, not when something the closure reads
  // changes underneath it.
  $: eventsForDay = (day: Date): EventWithParticipants[] =>
    filteredEvents.filter((event) => {
      const eventDate = new Date(event.start_time);
      return dateUtils.isSameDay(eventDate, day);
    });

  $: monthGrid = dateUtils.getMonthGrid(currentDate);
  $: weekDays = dateUtils.getWeekDays(currentDate);
  $: headerDate = (() => {
    if (view === 'day') {
      return currentDate.toLocaleDateString('en-US', {
        weekday: 'long',
        month: 'long',
        day: 'numeric',
        year: 'numeric'
      });
    } else {
      return currentDate.toLocaleDateString('en-US', { month: 'long', year: 'numeric' });
    }
  })();
</script>

<div class="anim-fade-up">
<CalendarHeader
  title={headerDate}
  {view}
  onPrev={prev}
  onNext={next}
  onToday={goToToday}
  onNewEvent={openCreateModal}
  on:view-change={handleViewChange}
/>

<div class="px-6">
  <div
    class="flex items-center gap-3 flex-wrap bg-white border border-line rounded-[11px] px-3.5 py-[11px] mb-3"
  >
    <span class="font-mono text-[10px] tracking-widest uppercase text-muted">Free tonight</span>
    {#if freeTonightError}
      <span class="text-xs text-red-600" role="alert">{freeTonightError}</span>
    {:else if freeFriends.length === 0}
      <span class="text-sm text-gray-500">No friends free right now</span>
    {:else}
      <div class="flex">
        {#each freeFriends as friend (friend.user_id)}
          {#if friend.avatar_url}
            <img
              src={friend.avatar_url}
              alt=""
              class="w-[26px] h-[26px] rounded-full border-2 border-white -mr-[7px]"
            />
          {:else}
            <div
              class="w-[26px] h-[26px] rounded-full bg-gray-300 border-2 border-white -mr-[7px] flex items-center justify-center text-[9px] font-bold text-white"
            >
              {friend.username.slice(0, 2).toUpperCase()}
            </div>
          {/if}
        {/each}
      </div>
      <span class="text-[13px]">{freeFriends.length} friends have nothing on</span>
    {/if}
    <button
      on:click={openCreateModal}
      class="ml-auto px-3 py-[7px] border border-primary text-primary rounded-lg text-xs font-semibold hover:bg-primary-hover"
    >
      Propose a time
    </button>
  </div>

  <div class="flex gap-2 flex-wrap mb-3">
    {#each filters as filter (filter.key)}
      <button
        on:click={() => (activeFilter = filter.key)}
        class="px-3 py-[7px] rounded-lg text-xs font-semibold border {activeFilter === filter.key
          ? 'border-primary bg-tint text-primary'
          : 'border-line bg-white text-muted'}"
      >
        {filter.label}
      </button>
    {/each}
  </div>
</div>

<div class="flex gap-4 items-start px-6 pb-6">
  <div class="flex-1 min-w-0">
    {#if view === 'month'}
      <MonthView
        bind:this={monthViewRef}
        {monthGrid}
        currentMonth={currentDate}
        {eventsForDay}
        onEventClick={selectEvent}
        on:showTooltip={handleShowTooltip}
        on:hideTooltip={handleHideTooltip}
      />

      <EventTooltip
        events={tooltipEvents}
        isVisible={tooltipVisible}
        position={tooltipPosition}
        on:mouseenter={handleTooltipMouseEnter}
        on:mouseleave={handleTooltipMouseLeave}
        on:refresh={handleRefresh}
      />
    {:else if view === 'week'}
      <WeekView {weekDays} {eventsForDay} onEventClick={selectEvent} />
    {:else}
      <DayView {currentDate} events={eventsForDay(currentDate)} onEventClick={selectEvent} />
    {/if}
  </div>

  <EventPeekPanel
    event={selectedEvent}
    on:refresh={handleRefresh}
    on:edit={(e) => openEditModal(e.detail)}
    on:deleted={handleDeleted}
  />
</div>
</div>

{#if showCreateModal}
  <CreateEventModal event={editingEvent} on:close={closeModal} on:saved={handleSaved} />
{/if}

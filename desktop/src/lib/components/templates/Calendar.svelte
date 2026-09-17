<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import type { BestSlot, EventWithParticipants, FriendInfo } from '$lib/types';
  import CalendarHeader from '$lib/components/molecules/CalendarHeader.svelte';
  import MonthView from '$lib/components/organisms/MonthView.svelte';
  import WeekView from '$lib/components/organisms/WeekView.svelte';
  import DayView from '$lib/components/organisms/DayView.svelte';
  import AgendaView from '$lib/components/organisms/AgendaView.svelte';
  import EventTooltip from '$lib/components/molecules/EventTooltip.svelte';
  import EventPeekPanel from '$lib/components/organisms/EventPeekPanel.svelte';
  import CreateEventModal from '$lib/components/CreateEventModal.svelte';
  import { dateUtils } from '$lib/utils/dateUtils';
  import { statusOf } from '$lib/utils/eventStatus';
  import { createEventDispatcher } from 'svelte';

  export let events: EventWithParticipants[] = [];

  const dispatch = createEventDispatcher();

  import type { ViewType } from '$lib/types';
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
  /**
   * A date to open the create form on, when it was started from a specific
   * day. Null for the header's "+ New event", which means "no opinion".
   * One nullable value rather than a second boolean beside `editingEvent` -
   * the same reason that one is nullable.
   */
  let initialDate: Date | null = null;

  function openCreateModal() {
    editingEvent = null;
    initialDate = null;
    showCreateModal = true;
  }

  function openEditModal(event: EventWithParticipants) {
    editingEvent = event;
    initialDate = null;
    showCreateModal = true;
  }

  function closeModal() {
    showCreateModal = false;
    editingEvent = null;
    // Cleared here too: otherwise the next "+ New event" opens on whatever
    // day was long-pressed before it.
    initialDate = null;
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

  onMount(() => {
    loadFreeTonight();
    loadBestSlot();
  });

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

  /**
   * The day whose events are listed under the grid, per the mobile mockup's
   * "Tap a day to filter the list under it".
   *
   * Lives here with `selectedEvent`, `activeFilter` and `currentDate` rather
   * than inside MonthView: it drives a list that is a sibling of the grid,
   * not a detail of it.
   */
  let selectedDay: Date | null = null;

  function toggleDay(day: Date) {
    // Tapping the selected day again clears it. Without this the only way
    // out of the filter is to find an empty day, and a busy month hasn't
    // got one.
    selectedDay = selectedDay && dateUtils.isSameDay(selectedDay, day) ? null : day;
  }

  // ⚠️ `selectedDay` and `eventsForDay` are both named directly in this
  // line. Svelte's reactive dependency tracking is static - it reads the
  // identifiers in the `$:` statement itself, not what a called function
  // transitively touches - and this codebase has been bitten by that three
  // times already (matchesFilter, eventsForDay, Frame's avatarUrl).
  $: selectedDayEvents = selectedDay ? eventsForDay(selectedDay) : [];

  /**
   * The mockup's "Best overlap this week: Fri 20:00, 7 free".
   *
   * Not fatal if it fails - the bar still says who is free now, which is all
   * it did before this existed.
   */
  let bestSlot: BestSlot | null = null;

  async function loadBestSlot() {
    try {
      const from = new Date();
      const to = new Date(from.getTime() + 7 * 24 * 60 * 60 * 1000);
      [bestSlot] = await api.getBestSlots(from, to, 120);
    } catch {
      bestSlot = null;
    }
  }

  /** "Propose a time", with the suggestion already in the form. */
  function proposeTime() {
    // The point of suggesting a slot is that the button applies it - a
    // suggestion the button ignores is decoration. There's a test for this.
    initialDate = bestSlot ? new Date(bestSlot.start) : null;
    editingEvent = null;
    showCreateModal = true;
  }

  /** Long press / double-click on a day: start an event on that date. */
  function startEventOn(day: Date) {
    // Keep the time of day sensible rather than midnight: a long press says
    // "something is happening this day", not "at 00:00".
    const start = new Date(day);
    start.setHours(19, 0, 0, 0);
    initialDate = start;
    editingEvent = null;
    showCreateModal = true;
  }

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

  <div>
    <div
      class="flex items-center gap-3 flex-wrap bg-surface border border-line rounded-[11px] px-3.5 py-[11px] mb-3"
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

      {#if bestSlot}
        <span class="hidden h-4 w-px bg-line sm:block"></span>
        <span class="text-[13px] text-muted">
          Best overlap this week:
          <strong class="font-semibold text-ink">
            {new Date(bestSlot.start).toLocaleString(undefined, {
              weekday: 'short',
              hour: '2-digit',
              minute: '2-digit'
            })}
          </strong>,
          {bestSlot.free_count} free
        </span>
      {/if}

      <button
        on:click={proposeTime}
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
            : 'border-line bg-surface text-muted'}"
        >
          {filter.label}
        </button>
      {/each}
    </div>
  </div>

  <div class="flex gap-4 items-start">
    <div class="flex-1 min-w-0">
      {#if view === 'month'}
        <MonthView
          bind:this={monthViewRef}
          {monthGrid}
          currentMonth={currentDate}
          {eventsForDay}
          {selectedDay}
          onDayClick={toggleDay}
          onDayLongPress={startEventOn}
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

        <!-- The mockup's "Tap a day to filter the list under it". Shown at
             every width: on a phone it is the only way to read a day whose
             cell is 52px wide, and on desktop it answers "what's on that
             day" without having to open each chip. -->
        {#if selectedDay}
          <section class="mt-4 anim-fade-up" aria-live="polite">
            <div class="mb-2 flex items-baseline gap-2">
              <h2 class="m-0 text-[15px] font-semibold">
                {selectedDay.toLocaleDateString(undefined, { weekday: 'long', day: 'numeric' })}
              </h2>
              <span class="font-mono text-[11px] text-muted">
                {selectedDayEvents.length}
                {selectedDayEvents.length === 1 ? 'event' : 'events'}
              </span>
              <button
                type="button"
                on:click={() => (selectedDay = null)}
                class="ml-auto text-[12px] font-semibold text-primary hover:underline"
              >
                Clear
              </button>
            </div>

            {#if selectedDayEvents.length > 0}
              <div class="flex flex-col gap-1.5">
                {#each selectedDayEvents as event (event.id)}
                  <button
                    type="button"
                    on:click={() => selectEvent(event)}
                    class="flex w-full items-center gap-2.5 rounded-[11px] border border-line bg-surface px-3 py-2.5 text-left hover:bg-subtle"
                  >
                    <span
                      class="h-[26px] w-[3px] shrink-0 rounded-full"
                      style="background:{statusOf(event.my_status).bar}"
                    ></span>
                    <span class="min-w-0 flex-1">
                      <span class="block truncate text-[14px] font-semibold">{event.title}</span>
                      <span class="block font-mono text-[11px] text-muted">
                        {new Date(event.start_time).toLocaleTimeString(undefined, {
                          hour: '2-digit',
                          minute: '2-digit'
                        })}{event.location ? ` · ${event.location}` : ''}
                      </span>
                    </span>
                  </button>
                {/each}
              </div>
            {:else}
              <p class="m-0 text-[13px] text-muted">
                Nothing on this day. Press and hold it — or double-click — to add something.
              </p>
            {/if}
          </section>
        {/if}
      {:else if view === 'week'}
        <WeekView {weekDays} {eventsForDay} onEventClick={selectEvent} />
      {:else if view === 'list'}
        <AgendaView events={filteredEvents} onEventClick={selectEvent} />
      {:else}
        <DayView events={eventsForDay(currentDate)} onEventClick={selectEvent} />
      {/if}
    </div>

    <EventPeekPanel
      event={selectedEvent}
      on:close={() => (selectedEvent = null)}
      on:refresh={handleRefresh}
      on:edit={(e) => openEditModal(e.detail)}
      on:deleted={handleDeleted}
    />
  </div>
</div>

{#if showCreateModal}
  <CreateEventModal
    event={editingEvent}
    {initialDate}
    on:close={closeModal}
    on:saved={handleSaved}
  />
{/if}

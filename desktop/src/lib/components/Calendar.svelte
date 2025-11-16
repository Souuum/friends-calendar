<script lang="ts">
  import type { EventWithParticipants } from "$lib/types";
  import { onMount } from "svelte";

  export let events: EventWithParticipants[] = [];

  // View states: "month" | "week" | "day"
  let view: "month" | "week" | "day" = "month";

  let currentDate = new Date();

  function startOfWeek(date: Date) {
    const d = new Date(date);
    const day = d.getDay();
    d.setDate(d.getDate() - day);
    return d;
  }

  function startOfMonth(date: Date) {
    return new Date(date.getFullYear(), date.getMonth(), 1);
  }

  // Move in time depending on view
  function prev() {
    if (view === "month") {
      currentDate = new Date(currentDate.getFullYear(), currentDate.getMonth() - 1, 1);
    } else if (view === "week") {
      currentDate = new Date(currentDate.setDate(currentDate.getDate() - 7));
    } else {
      currentDate = new Date(currentDate.setDate(currentDate.getDate() - 1));
    }
  }

  function next() {
    if (view === "month") {
      currentDate = new Date(currentDate.getFullYear(), currentDate.getMonth() + 1, 1);
    } else if (view === "week") {
      currentDate = new Date(currentDate.setDate(currentDate.getDate() + 7));
    } else {
      currentDate = new Date(currentDate.setDate(currentDate.getDate() + 1));
    }
  }

  // Filter events for a given day
  function eventsForDay(day: Date) {
    return events.filter(event => {
      const eventDate = new Date(event.start_time);
      return (
        eventDate.getFullYear() === day.getFullYear() &&
        eventDate.getMonth() === day.getMonth() &&
        eventDate.getDate() === day.getDate()
      );
    });
  }

  // Build month grid
  function getMonthGrid() {
    const start = startOfMonth(currentDate);
    const firstDayIndex = start.getDay();

    const gridStart = new Date(start);
    gridStart.setDate(start.getDate() - firstDayIndex);

    return Array.from({ length: 42 }, (_, i) => {
      const d = new Date(gridStart);
      d.setDate(gridStart.getDate() + i);
      return d;
    });
  }

  // Build week
  function getWeekDays() {
    const start = startOfWeek(currentDate);
    return Array.from({ length: 7 }, (_, i) => {
      const d = new Date(start);
      d.setDate(start.getDate() + i);
      return d;
    });
  }
</script>

<!-- Header Navigation -->
<div class="flex items-center justify-between mb-4">
  <div class="flex gap-2">
    <button class="px-3 py-1 rounded bg-gray-100" on:click={prev}>←</button>
    <button class="px-3 py-1 rounded bg-gray-100" on:click={next}>→</button>
  </div>

  <h2 class="text-xl font-bold">
    {currentDate.toLocaleDateString("en-US", {
      month: "long",
      year: "numeric"
    })}
  </h2>

  <select bind:value={view} class="px-2 py-1 border rounded">
    <option value="month">Month</option>
    <option value="week">Week</option>
    <option value="day">Day</option>
  </select>
</div>

<!-- MONTH VIEW -->
{#if view === "month"}
  <div class="grid grid-cols-7 text-center text-gray-600 font-semibold mb-2">
    <div>Sun</div><div>Mon</div><div>Tue</div><div>Wed</div><div>Thu</div><div>Fri</div><div>Sat</div>
  </div>

  <div class="grid grid-cols-7 gap-1">
    {#each getMonthGrid() as day}
      <div class="border p-1 h-28 rounded bg-white hover:bg-gray-50 transition">
        <div class="text-sm font-medium mb-1">{day.getDate()}</div>

        <div class="space-y-1 overflow-y-auto h-20">
          {#each eventsForDay(day) as event}
          <!-- if an event match, console.log it-->
           {#if event}
            {console.log("event for day :", event)}
            {:else}
            {console.log("no event for day")}
           {/if}
           
            <slot name="event" {event}></slot>
          {/each}
        </div>
      </div>
    {/each}
  </div>
{/if}

<!-- WEEK VIEW -->
{#if view === "week"}
  <div class="grid grid-cols-7 text-center text-gray-600 font-semibold mb-2">
    {#each getWeekDays() as day}
      <div>{day.toLocaleDateString("en-US", { weekday: "short" })}</div>
    {/each}
  </div>

  <div class="grid grid-cols-7 gap-1">
    {#each getWeekDays() as day}
      <div class="border p-1 h-48 rounded bg-white hover:bg-gray-50 transition">
        <div class="text-sm font-medium mb-1">{day.getDate()}</div>
        
        <div class="space-y-1 overflow-y-auto h-40">
          {#each eventsForDay(day) as event}
            <slot name="event" {event}></slot>
          {/each}
        </div>
      </div>
    {/each}
  </div>
{/if}

<!-- DAY VIEW -->
{#if view === "day"}
  <div class="border rounded p-3 bg-white">
    <h3 class="font-bold text-lg mb-3">
      {currentDate.toLocaleDateString("en-US", { weekday: "long", month: "long", day: "numeric" })}
    </h3>

    <div class="space-y-3">
      {#each eventsForDay(currentDate) as event}
        <slot name="event" {event}></slot>
      {/each}

      {#if eventsForDay(currentDate).length === 0}
        <p class="text-gray-500 text-sm">No events today.</p>
      {/if}
    </div>
  </div>
{/if}

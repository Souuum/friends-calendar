<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import { fn } from 'storybook/test';
  import CalendarHeader from '$lib/components/molecules/CalendarHeader.svelte';

  /**
   * The calendar's toolbar: previous and next arrows, the title, a "Today"
   * button, the view switcher and "+ New Event". Use it above a calendar grid.
   *
   * ```svelte
   * <CalendarHeader
   *   title={headerDate}
   *   {view}
   *   onPrev={prev}
   *   onNext={next}
   *   onToday={goToToday}
   *   onNewEvent={openCreateModal}
   *   on:view-change={(e) => (view = e.detail)}
   * />
   * ```
   *
   * - It holds no state: the caller owns the date and the view, and builds
   *   `title` itself.
   * - Forwards `view-change` from the switcher, with the chosen view as `detail`.
   */
  const { Story } = defineMeta({
    title: 'Components/Molecules/CalendarHeader',
    component: CalendarHeader,
    argTypes: { view: { control: 'inline-radio', options: ['month', 'week', 'day', 'list'] } },
    args: {
      title: 'October 2026',
      view: 'month',
      onPrev: fn(),
      onNext: fn(),
      onToday: fn(),
      onNewEvent: fn()
    }
  });
</script>

<!-- `view="month"` with a month title. -->
<Story name="Month" />

<!-- `view="week"` with a date range as the title. -->
<Story name="Week" args={{ view: 'week', title: 'Oct 19 – 25, 2026' }} />

<!-- `view="day"` with a full date, the longest title. -->
<Story name="Day" args={{ view: 'day', title: 'Monday, October 19, 2026' }} />

<!-- `view="list"` highlights the list option in the switcher. -->
<Story name="List" args={{ view: 'list' }} />

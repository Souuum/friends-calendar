<script lang="ts">
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
  import { unreadNotificationCount } from '$lib/stores';

  // Mirrors MobileTabBar.dc.html's 5 tabs. Distinct from Frame.svelte's
  // sidebar navItems: the mobile mockup folds "Discord server" under this
  // "Me" tab (reached from within /settings) instead of giving it its own
  // tab, so this list isn't just navItems with different labels - see
  // .claude/skills/mockup-responsive-shell/SKILL.md.
  $: tabs = [
    { label: 'Calendar', icon: '📅', view: '/' },
    { label: 'Friends', icon: '👥', view: '/friends' },
    { label: 'Hub', icon: '🔔', view: '/announcements' },
    { label: 'Alerts', icon: '🔔', view: '/notifications', badge: $unreadNotificationCount },
    { label: 'Me', icon: '⚙️', view: '/settings' }
  ];
</script>

<nav
  data-testid="bottom-tab-bar"
  class="md:hidden fixed bottom-0 inset-x-0 z-30 flex items-start gap-0.5 bg-white border-t border-gray-200 px-2 pt-2"
>
  {#each tabs as tab}
    {@const current = $page.url.pathname === tab.view}
    <!-- The bar's bottom safe-area padding lives on the buttons, not on the
         nav: identical pixels either way, but this way it counts toward the
         tap target. On the nav it was dead space and the buttons were 37px,
         under the 44px floor. -->
    <button
      on:click={() => goto(tab.view)}
      class="flex-1 min-w-0 flex flex-col items-center gap-1 relative pt-0.5 pb-[26px]"
    >
      <span class="text-lg leading-none">{tab.icon}</span>
      <span
        class="text-[11px] leading-none {current
          ? 'font-bold text-primary'
          : 'font-medium text-gray-500'}"
      >
        {tab.label}
      </span>
      {#if tab.badge}
        <span
          class="absolute -top-1 right-1/4 min-w-[16px] h-4 rounded-full bg-secondary text-white text-[10px] font-bold flex items-center justify-center px-1"
        >
          {tab.badge}
        </span>
      {/if}
    </button>
  {/each}
</nav>

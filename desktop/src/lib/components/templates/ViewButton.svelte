<script lang="ts">
  export let item: { label: string; view: string; badge?: number };
  export let current: boolean = false;
</script>

<!--
  Sidebar row, matching the mockup's own nav button: a 6px dot, the label,
  and a mono badge - no icon.

  ⚠️ The dot replaced an SVG icon here deliberately, and only here. The
  mockup's desktop sidebar identifies the active row by a filled accent dot
  plus a tint pill; `MobileTabBar.dc.html` *does* use icons, so
  `BottomTabBar` keeps them. Two navigations, two designs.

  Values (216px rail, 9px radius, 13px text, 9x10 padding, 6px dot, mono
  10px badge) are the mockup's, not approximations.
-->
<button
  on:click
  class="flex w-full items-center gap-2.5 rounded-[9px] px-2.5 py-[9px] text-left text-[13px]
        {current ? 'bg-tint text-primary font-semibold' : 'text-ink font-medium hover:bg-subtle'}"
  aria-current={current ? 'page' : undefined}
>
  <span
    class="h-1.5 w-1.5 shrink-0 rounded-full {current ? 'bg-primary' : 'bg-line'}"
    aria-hidden="true"
  ></span>
  <span class="flex-1">{item.label}</span>
  {#if item.badge}
    <span class="font-mono text-[10px] {current ? 'text-primary' : 'text-muted'}">
      {item.badge}
    </span>
  {/if}
</button>

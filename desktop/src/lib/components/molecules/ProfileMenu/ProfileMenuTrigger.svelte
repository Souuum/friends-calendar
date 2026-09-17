<script lang="ts">
  import Avatar from '$lib/components/atoms/Avatar.svelte';
  import ChevronDown from '$lib/components/icons/ChevronDown.svelte';
  import ChevronUp from '$lib/components/icons/ChevronUp.svelte';
  export let username = '';
  export let avatar = '';
  export let show = false;
</script>

<div class="flex min-w-0 select-none items-center gap-2.5 border-none bg-none">
  <Avatar src={avatar} size={32} on:click class="cursor-pointer" />
  <!-- 14px/600 per the mockup. This was `text-xl`, which made the handle the
       largest thing on every screen - louder than the page title. Truncates
       because a Discord handle has no length limit worth trusting. -->
  <span class="hidden min-w-0 max-w-[180px] truncate text-[14px] font-semibold sm:block"
    >{username}</span
  >
  <button
    class="p-1 hover:bg-gray-100 rounded-lg transition cursor-pointer"
    on:click|stopPropagation={() => (show = !show)}
  >
    {#if show}
      <ChevronUp />
    {:else}
      <ChevronDown />
    {/if}
  </button>
</div>

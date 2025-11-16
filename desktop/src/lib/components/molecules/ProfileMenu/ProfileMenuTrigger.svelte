<script lang="ts">
  import Avatar from '$lib/components/atoms/Avatar.svelte';
  import ChevronDown from '$lib/components/icons/ChevronDown.svelte';
  export let username = "";
  export let avatar = "";
  export let onClick = () => {};
</script>

<div class="flex items-center gap-2 cursor-pointer select-none bg-none border-none">
  <Avatar src={avatar} size={48} />
  <span class="p-1 text-black font-bold text-xl">{username}</span>

<button
  class="p-1 hover:bg-gray-100 rounded-lg transition"
  on:click={onClick}
>
  <ChevronDown />
</button>
</div>
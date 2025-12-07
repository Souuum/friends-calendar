<script>
  import ViewSwitcher from "$lib/components/atoms/ViewSwitcher.svelte";
    export let view

</script>

<ViewSwitcher bind:view />

<pre>view : {view}</pre>
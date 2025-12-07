<script>
    import ProfileMenu from "$lib/components/molecules/ProfileMenu/ProfileMenu.svelte";
</script>

<ProfileMenu on:logout={() => alert("logout !")} on:settings={() => alert("settings !")}/>
<script>
  /**
   * Avatar src
   */
  export let src = '';
  /**
   * Size
   */
  export let size = 48;

  /**
   * Extra class
   */
  export { className as class }
  let className = ""
</script>

<button on:click class={className}>
  <img {src} alt="User avatar" class="rounded-full" style={`width:${size}px; height:${size}px;`} />
</button>

<style>
  button {
    background-color: inherit;
    border: inherit;
  }
</style>

<script lang="ts">
  export let name = "world";
  let count = 0;
  $: doubled = count * 2;
</script>

<h1>Hello {name}!</h1>
<button on:click={() => count++} disabled={count > 9}>
  Clicked {count} {count === 1 ? "time" : "times"}
</button>

{#if doubled > 10}
  <p class="warn">That is a lot of clicks.</p>
{:else}
  <p>Doubled: {doubled}</p>
{/if}

{#each ["a", "b"] as letter, i}
  <span>{i}: {letter}</span>
{/each}

<style>
  h1 { color: #ff3e00; }
</style>

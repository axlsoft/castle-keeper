<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  let name = "";
  let greetMsg = "";

  async function greet(event: Event) {
    event.preventDefault();
    try {
      greetMsg = await invoke("greet", { name });
    } catch (error: any) {
      greetMsg = error.message;
    }
  }
</script>

<main class="container">
  <h1>Castle Keeper</h1>

  <form on:submit={greet}>
    <input type="text" bind:value={name} placeholder="Enter your name" />
    <button type="submit">Greet</button>
  </form>

  <p>{greetMsg}</p>

  <a href="/settings">Go to Settings</a>
</main>

<style>
  .container {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100vh;
    text-align: center;
  }

  input,
  button {
    margin: 0.5em;
    padding: 0.5em;
    font-size: 1em;
  }

  button {
    cursor: pointer;
  }
</style>

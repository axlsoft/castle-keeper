<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import Input from "@/components/ui/input/input.svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { toast } from "svelte-sonner";

  let name = "";

  async function greet(event: Event) {
    event.preventDefault();
    try {
      const greetMsg: string = await invoke("greet", { name });
      toast.success(greetMsg);
      name = "";
    } catch (error: any) {
      toast.error(error);
    }
  }
</script>

<main
  class="container mx-auto flex flex-col items-center justify-center h-screen text-center"
>
  <form onsubmit={greet} class="flex w-full max-w-sm items-center space-x-2">
    <Input type="text" bind:value={name} placeholder="Enter your name" />
    <Button type="submit">Greet</Button>
  </form>
</main>

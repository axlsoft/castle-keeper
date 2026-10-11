<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import Input from "@/components/ui/input/input.svelte";
  import { commands } from "@/bindings";
  import { toastError } from "@/ipc-service";
  import { toast } from "svelte-sonner";

  let topic = $state("");
  let key = $state("");
  let value = $state("");

  async function produce(event: Event) {
    event.preventDefault();
    try {
      const result = await commands.produceMessage({ topic, key: key || null, value });
      toast.success(`Produced to ${topic} partition ${result.partition} at offset ${result.offset}`);
      value = "";
    } catch (error) {
      toastError(error);
    }
  }
</script>

<main class="container mx-auto flex h-screen flex-col items-center justify-center gap-4">
  <h1 class="text-lg font-semibold">Produce a test message</h1>
  <form onsubmit={produce} class="flex w-full max-w-md flex-col gap-2">
    <Input type="text" bind:value={topic} placeholder="Topic" required />
    <Input type="text" bind:value={key} placeholder="Key (optional)" />
    <Input type="text" bind:value={value} placeholder="Value" required />
    <Button type="submit">Produce</Button>
  </form>
</main>

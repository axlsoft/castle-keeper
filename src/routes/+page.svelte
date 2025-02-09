<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import Input from "@/components/ui/input/input.svelte";
  import { send_ipc } from "@/ipc-service";
  import {
    KafkaMessageType,
    type KafkaMessageRequest,
    type KafkaMessageResponse,
  } from "@/model/kafka_message";

  import { toast } from "svelte-sonner";

  let name = "";

  async function greet(event: Event) {
    event.preventDefault();
    const m: KafkaMessageRequest = {
      key: "greet",
      value: name,
      metadata: {
        message_type: KafkaMessageType.Json,
      },
    };
    try {
      const publishResult: KafkaMessageResponse = await send_ipc("send_message", {message:m});
      const successMessage = `Created new message at ${publishResult.offset} on partition ${publishResult.partition} `;
      toast.success(successMessage);
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

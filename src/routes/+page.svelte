<script lang="ts">
  import { onMount } from "svelte";
  import { Button } from "$lib/components/ui/button";
  import Input from "@/components/ui/input/input.svelte";
  import TypedConfirmDialog from "$lib/components/app/typed-confirm-dialog.svelte";
  import { commands, type ProduceOutcome } from "@/bindings";
  import { environmentStyle } from "@/environments";
  import { toAppError, toastError, type AppError } from "@/ipc-service";
  import { connections } from "@/stores/connections.svelte";
  import { toast } from "svelte-sonner";

  let topic = $state("");
  let key = $state("");
  let value = $state("");
  let dryRun = $state(false);

  let blocked = $state<AppError | null>(null);
  let preview = $state<ProduceOutcome | null>(null);
  let unlockOpen = $state(false);
  let confirmOpen = $state(false);

  onMount(() => {
    connections.refresh().catch(toastError);
  });

  const active = $derived(connections.active);
  const readOnly = $derived(active ? connections.isReadOnly(active) : false);

  async function send(confirmation: string | null) {
    blocked = null;
    preview = null;
    try {
      const outcome = await commands.produceMessage({ topic, key: key || null, value }, dryRun, confirmation);
      if (outcome.dryRun) {
        preview = outcome;
      } else {
        toast.success(`Produced to ${outcome.topic} · partition ${outcome.partition} · offset ${outcome.offset}`);
        value = "";
      }
    } catch (e) {
      const error = toAppError(e);
      if (error.kind === "readOnly") blocked = error;
      else if (error.kind === "confirmationRequired") confirmOpen = true;
      else toastError(error);
    }
  }

  function submit(event: Event) {
    event.preventDefault();
    send(null);
  }
</script>

<main class="container mx-auto flex max-w-lg flex-col gap-4 py-10">
  <div class="flex items-center justify-between">
    <h1 class="text-lg font-semibold">Produce a message</h1>
    {#if active}
      {@const env = environmentStyle(active.environment)}
      <span class="flex items-center gap-2 text-sm">
        <span class="h-3 w-1.5 rounded-sm {env.band}"></span>{active.name}
        {#if readOnly}<span class="text-muted-foreground text-[10px] font-semibold uppercase">Read-only</span>{/if}
      </span>
    {/if}
  </div>

  {#if blocked && active}
    <div class="border-destructive/50 bg-destructive/5 rounded-md border p-3 text-sm" role="alert">
      <p class="text-destructive font-medium">Write blocked: {blocked.message}</p>
      {#if blocked.hint}<p class="text-muted-foreground mt-1">{blocked.hint}</p>{/if}
      <p class="text-muted-foreground mt-1 text-xs">This attempt was recorded in the action log.</p>
      <Button size="sm" variant="outline" class="mt-2" onclick={() => (unlockOpen = true)}>
        Unlock for this session…
      </Button>
    </div>
  {/if}

  <form onsubmit={submit} class="flex flex-col gap-2">
    <Input type="text" bind:value={topic} placeholder="Topic" required class="font-mono" />
    <Input type="text" bind:value={key} placeholder="Key (optional)" class="font-mono" />
    <textarea
      bind:value={value}
      placeholder="Value"
      required
      rows="5"
      class="border-input bg-background rounded-md border px-3 py-2 font-mono text-sm"
    ></textarea>
    <label class="flex items-center gap-2 text-sm">
      <input type="checkbox" bind:checked={dryRun} />
      Dry run (validate and preview, don't send)
    </label>
    <Button type="submit" disabled={!active}>{dryRun ? "Preview" : "Produce"}</Button>
  </form>

  {#if preview}
    <div class="rounded-md border p-3 text-sm" role="status">
      <p class="font-medium">Dry run — nothing was sent</p>
      <p class="text-muted-foreground mt-1 font-mono text-xs tabular-nums">
        {preview.topic} · partition {preview.partition ?? "auto"} · key {preview.keyBytes} B · value
        {preview.valueBytes} B · {preview.headerCount} headers
      </p>
    </div>
  {/if}
</main>

{#if active}
  <TypedConfirmDialog
    bind:open={unlockOpen}
    title="Unlock {active.name} for this session?"
    description="Writes will be allowed until you switch connections or restart the app. The unlock is recorded in the action log."
    expected={active.name}
    confirmLabel="Unlock"
    onconfirm={async (typed) => {
      await connections.unlock(active.id, typed).catch(toastError);
      blocked = null;
    }}
  />
  <TypedConfirmDialog
    bind:open={confirmOpen}
    title="Write to {active.name}?"
    description="This is a production connection. Confirm this produce to {topic}."
    expected={active.name}
    confirmLabel="Produce"
    onconfirm={(typed) => send(typed)}
  />
{/if}

<script lang="ts">
    import { onMount } from "svelte";
    import { commands } from "@/bindings";
    import { toAppError, type AppError } from "@/ipc-service";

    let topics = $state<string[]>([]);
    let error = $state<AppError | null>(null);

    onMount(async () => {
        try {
            topics = await commands.listTopics(false);
        } catch (e) {
            error = toAppError(e);
        }
    });
</script>

<main class="p-6">
    <h1 class="text-lg font-semibold">Topics</h1>
    {#if error}
        <p class="text-destructive mt-2 text-sm">{error.message}</p>
        {#if error.hint}<p class="text-muted-foreground mt-1 text-sm">{error.hint}</p>{/if}
    {:else}
        <ul class="mt-4 font-mono text-sm">
            {#each topics as topic (topic)}
                <li>{topic}</li>
            {/each}
        </ul>
    {/if}
</main>

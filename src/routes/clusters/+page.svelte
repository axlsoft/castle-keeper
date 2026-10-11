<script lang="ts">
    import { onMount } from "svelte";
    import { send_ipc } from "@/ipc-service";

    let topics = $state<string[]>([]);
    let error = $state<string | null>(null);

    onMount(async () => {
        try {
            topics = await send_ipc<string[]>("get_topics");
        } catch (e) {
            error = String(e);
        }
    });
</script>

<main class="container py-8">
    <h1 class="text-lg font-semibold">Cluster Management</h1>
    {#if error}
        <p class="text-destructive mt-2 text-sm">{error}</p>
    {:else}
        <ul class="mt-4 font-mono text-sm">
            {#each topics as topic (topic)}
                <li>{topic}</li>
            {/each}
        </ul>
    {/if}
</main>

<script lang="ts">
    import { onMount } from "svelte";
    import type { PageData } from "./$types";
    import { send_ipc } from "@/ipc-service";

    let { data }: { data: PageData } = $props();

    let topics = $state([]);

    onMount(async () => {
        const result: [] = await send_ipc("get_topics");
        topics = result;
    });
</script>

<main>
    <h1>Clusters</h1>
    <ul>
        {#each topics as topic}
            <li>{topic}</li>
        {/each}
    </ul>
</main>

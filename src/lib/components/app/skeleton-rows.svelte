<script lang="ts">
    let { rows = 8, height = 32 }: { rows?: number; height?: number } = $props();
</script>

<div class="flex flex-col gap-1 p-2" aria-busy="true" aria-label="Loading">
    {#each Array(rows) as _, i (i)}
        <div class="bg-muted animate-pulse rounded" style="height: {height - 8}px; opacity: {1 - i / (rows + 2)}"></div>
    {/each}
</div>

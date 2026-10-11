<script lang="ts">
    import { environmentStyle } from "#lib/environments.js";
    import { connections } from "#lib/stores/connections.svelte.js";

    const active = $derived(connections.active);
    const env = $derived(active ? environmentStyle(active.environment) : null);
    const readOnly = $derived(active ? connections.isReadOnly(active) : false);
    const unlocked = $derived(active ? active.readOnly && !readOnly : false);
</script>

<!-- Always visible: which cluster, which environment, whether writes are possible. -->
<div
    class="flex h-6 shrink-0 items-center justify-center gap-2 text-[11px] font-semibold tracking-wide text-white uppercase select-none {env?.band ??
        'bg-zinc-400 dark:bg-zinc-700'}"
    data-tauri-drag-region
    role="status"
    aria-label="Active connection"
>
    {#if active && env}
        <span>{env.label}</span>
        <span aria-hidden="true">·</span>
        <span class="normal-case">{active.name}</span>
        <span aria-hidden="true">·</span>
        {#if unlocked}
            <span class="rounded bg-white/25 px-1">Unlocked for this session</span>
        {:else if readOnly}
            <span>Read-only</span>
        {:else}
            <span>Writable</span>
        {/if}
    {:else}
        <span>No connection</span>
    {/if}
</div>

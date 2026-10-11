<script lang="ts">
    import { untrack } from "svelte";
    import JsonTree from "./json-tree.svelte";

    interface Props {
        value: unknown;
        name?: string | number | null;
        depth?: number;
        /** Nodes deeper than this start collapsed. */
        openDepth?: number;
    }

    let { value, name = null, depth = 0, openDepth = 2 }: Props = $props();

    const isArray = $derived(Array.isArray(value));
    const isObject = $derived(value !== null && typeof value === "object");
    const entries = $derived(
        isObject ? (isArray ? (value as unknown[]).map((v, i) => [i, v] as const) : Object.entries(value as object)) : [],
    );
    // Initial expansion only; the user toggles it from then on.
    let open = $state(untrack(() => depth < openDepth));

    function scalarClass(v: unknown) {
        if (typeof v === "string") return "text-emerald-700 dark:text-emerald-400";
        if (typeof v === "number") return "text-blue-700 dark:text-blue-300";
        if (typeof v === "boolean") return "text-purple-700 dark:text-purple-300";
        return "text-muted-foreground";
    }
</script>

<div class="font-mono text-xs leading-5" style="padding-left: {depth ? 14 : 0}px">
    {#if isObject}
        <button type="button" class="hover:bg-accent rounded px-0.5 text-left" onclick={() => (open = !open)} aria-expanded={open}>
            <span class="text-muted-foreground inline-block w-3">{open ? "▾" : "▸"}</span>{#if name !== null}<span class="text-foreground/80">{typeof name === "number" ? name : JSON.stringify(name)}</span>: {/if}<span class="text-muted-foreground">{isArray ? `[${entries.length}]` : `{${entries.length}}`}</span>
        </button>
        {#if open}
            {#each entries as [k, v] (k)}
                <JsonTree value={v} name={k} depth={depth + 1} {openDepth} />
            {/each}
        {/if}
    {:else}
        <span class="inline-block w-3"></span>{#if name !== null}<span class="text-foreground/80">{typeof name === "number" ? name : JSON.stringify(name)}</span>: {/if}<span class="break-all {scalarClass(value)}">{JSON.stringify(value)}</span>
    {/if}
</div>

<script lang="ts">
    import { goto } from "$app/navigation";
    import RefreshCw from "@lucide/svelte/icons/refresh-cw";
    import Search from "@lucide/svelte/icons/search";
    import { Button } from "$lib/components/ui/button";
    import ErrorState from "$lib/components/app/error-state.svelte";
    import SkeletonRows from "$lib/components/app/skeleton-rows.svelte";
    import VirtualList from "$lib/components/app/virtual-list.svelte";
    import CopyCli from "$lib/components/app/copy-cli.svelte";
    import { topicListCommands } from "@/cli";
    import type { TopicSummary } from "@/bindings";
    import { ago } from "@/stores/cache.svelte";
    import { connections } from "@/stores/connections.svelte";
    import { paletteCommands } from "@/stores/palette.svelte";
    import { topicsCache } from "@/stores/topics.svelte";
    import { fuzzyFilter } from "@/util/fuzzy";

    const ROW = 32;

    let query = $state("");
    let showInternal = $state(false);
    let selected = $state(-1);
    let search: HTMLInputElement | undefined = $state();
    let list: ReturnType<typeof VirtualList> | undefined = $state();
    let now = $state(Date.now());

    const key = $derived(connections.activeId);
    const entry = $derived(key ? topicsCache.entry(key) : null);
    $effect(() => {
        if (key) topicsCache.ensure(key);
    });
    const all = $derived(entry?.data ?? []);
    const visible = $derived(fuzzyFilter(showInternal ? all : all.filter((t) => !t.internal), query, (t) => t.name));
    const hiddenInternal = $derived(all.filter((t) => t.internal).length);
    const unhealthy = $derived(all.filter((t) => t.underReplicated > 0 || t.offline > 0).length);

    $effect(() => {
        const timer = setInterval(() => (now = Date.now()), 5000);
        return () => clearInterval(timer);
    });

    // Reset the keyboard selection when the filter changes.
    $effect(() => {
        void query;
        void showInternal;
        selected = visible.length ? 0 : -1;
    });

    // Offer every topic in the command palette while this page is open.
    $effect(() =>
        paletteCommands.set(
            "Topics",
            all.slice(0, 5000).map((t) => ({
                id: `topic:${t.name}`,
                label: t.name,
                detail: `${t.partitions}p`,
                run: () => open(t),
            })),
        ),
    );

    function open(topic: TopicSummary) {
        goto(`/topic?name=${encodeURIComponent(topic.name)}`);
    }

    function onkeydown(event: KeyboardEvent) {
        const typing = event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement;
        if (event.key === "/" && !typing) {
            event.preventDefault();
            search?.focus();
        } else if (event.key === "Escape" && event.target === search) {
            query = "";
            search?.blur();
        } else if (event.target === search && (event.key === "ArrowDown" || event.key === "Enter")) {
            event.preventDefault();
            if (event.key === "Enter" && visible[selected]) open(visible[selected]);
            else list?.focus();
        }
    }
</script>

<svelte:window {onkeydown} />

<main class="flex h-full flex-col">
    <header class="flex items-center gap-3 border-b px-6 py-3">
        <h1 class="text-lg font-semibold">Topics</h1>
        {#if entry?.data}
            <span class="text-muted-foreground text-sm tabular-nums">
                {visible.length.toLocaleString()}{query || !showInternal ? ` of ${all.length.toLocaleString()}` : ""}
            </span>
            {#if unhealthy}
                <span class="rounded bg-amber-500/15 px-1.5 py-0.5 text-xs font-medium text-amber-700 dark:text-amber-300">
                    {unhealthy} under-replicated or offline
                </span>
            {/if}
        {/if}
        <div class="ml-auto flex items-center gap-2">
            <span class="text-muted-foreground text-xs" title="Topics are cached per connection">
                {entry?.loading ? "Refreshing…" : `Updated ${ago(entry?.fetchedAt ?? null, now)}`}
            </span>
            {#if connections.active}<CopyCli commands={topicListCommands(connections.active)} />{/if}
            <Button size="sm" variant="ghost" onclick={() => key && topicsCache.refresh(key)} disabled={!key || entry?.loading} aria-label="Refresh topics">
                <RefreshCw class="size-4 {entry?.loading ? 'animate-spin' : ''}" />
            </Button>
        </div>
    </header>

    <div class="flex items-center gap-3 border-b px-6 py-2">
        <label class="relative flex-1">
            <Search class="text-muted-foreground absolute top-2 left-2 size-4" />
            <input
                bind:this={search}
                bind:value={query}
                placeholder="Filter topics  ( / )"
                class="bg-background h-8 w-full rounded-md border pr-2 pl-8 font-mono text-sm outline-none focus:ring-1"
                aria-label="Filter topics"
            />
        </label>
        <label class="text-muted-foreground flex items-center gap-1.5 text-sm whitespace-nowrap">
            <input type="checkbox" bind:checked={showInternal} />
            Internal topics{hiddenInternal && !showInternal ? ` (${hiddenInternal})` : ""}
        </label>
    </div>

    <div class="text-muted-foreground grid grid-cols-[1fr_7rem_4rem_9rem] gap-2 border-b px-6 py-1.5 text-xs font-medium">
        <span>Name</span>
        <span class="text-right">Partitions</span>
        <span class="text-right">RF</span>
        <span>Health</span>
    </div>

    <div class="min-h-0 flex-1">
        {#if !key}
            <div class="text-muted-foreground mt-16 text-center text-sm">
                <p>No connection selected.</p>
                <Button class="mt-3" size="sm" onclick={() => goto("/connections")}>Add a connection</Button>
            </div>
        {:else if entry?.error && !entry.data}
            <ErrorState error={entry.error} onretry={() => topicsCache.refresh(key)} />
        {:else if !entry?.data}
            <SkeletonRows height={ROW} />
        {:else if visible.length === 0}
            <div class="text-muted-foreground mt-16 text-center text-sm">
                {#if query}
                    <p>No topics match “{query}”.</p>
                    <Button class="mt-3" size="sm" variant="outline" onclick={() => (query = "")}>Clear filter</Button>
                {:else}
                    <p>This cluster has no topics yet.</p>
                    <Button class="mt-3" size="sm" variant="outline" onclick={() => goto("/produce")}>Produce a message</Button>
                {/if}
            </div>
        {:else}
            <VirtualList
                bind:this={list}
                items={visible}
                rowHeight={ROW}
                bind:selected
                onactivate={(t) => open(t)}
                class="h-full"
            >
                {#snippet row(topic: TopicSummary, _index: number, isSelected: boolean)}
                    <div
                        class="grid h-full cursor-default grid-cols-[1fr_7rem_4rem_9rem] items-center gap-2 px-6 text-sm {isSelected
                            ? 'bg-accent'
                            : 'hover:bg-accent/50'}"
                    >
                        <span class="truncate font-mono" title={topic.name}>
                            {topic.name}
                            {#if topic.internal}<span class="text-muted-foreground ml-1 text-[10px] uppercase">internal</span>{/if}
                        </span>
                        <span class="text-right tabular-nums">{topic.partitions}</span>
                        <span class="text-right tabular-nums">{topic.replicationFactor}</span>
                        <span>
                            {#if topic.offline}
                                <span class="text-destructive text-xs font-medium">{topic.offline} offline</span>
                            {:else if topic.underReplicated}
                                <span class="rounded bg-amber-500/15 px-1 text-xs font-medium text-amber-700 dark:text-amber-300">
                                    {topic.underReplicated} under-replicated
                                </span>
                            {:else}
                                <span class="text-muted-foreground text-xs">ok</span>
                            {/if}
                        </span>
                    </div>
                {/snippet}
            </VirtualList>
        {/if}
    </div>
</main>

<script lang="ts">
    import { goto } from "$app/navigation";
    import RefreshCw from "@lucide/svelte/icons/refresh-cw";
    import { Button } from "$lib/components/ui/button";
    import ErrorState from "$lib/components/app/error-state.svelte";
    import SkeletonRows from "$lib/components/app/skeleton-rows.svelte";
    import VirtualList from "$lib/components/app/virtual-list.svelte";
    import type { GroupSummary } from "@/bindings";
    import { ACTIVE_STATES, formatLag, lagClass } from "@/lag";
    import { ago } from "@/stores/cache.svelte";
    import { connections } from "@/stores/connections.svelte";
    import { groupsCache } from "@/stores/groups.svelte";
    import { paletteCommands } from "@/stores/palette.svelte";
    import { fuzzyFilter } from "@/util/fuzzy";

    type SortKey = "groupId" | "state" | "members" | "totalLag";
    type Filter = "all" | "active" | "empty";

    const ROW = 32;
    let query = $state("");
    let filter = $state<Filter>("all");
    let sort = $state<{ key: SortKey; desc: boolean }>({ key: "totalLag", desc: true });
    let selected = $state(-1);
    let search: HTMLInputElement | undefined = $state();
    let list: ReturnType<typeof VirtualList> | undefined = $state();

    const key = $derived(connections.activeId);
    const entry = $derived(key ? groupsCache.entry(key) : null);
    $effect(() => {
        if (key) groupsCache.ensure(key);
    });

    const all = $derived(entry?.data ?? []);
    const counts = $derived({
        all: all.length,
        active: all.filter((g) => ACTIVE_STATES.has(g.state)).length,
        empty: all.filter((g) => g.state === "Empty").length,
    });
    const visible = $derived.by(() => {
        let groups = all;
        if (filter === "active") groups = groups.filter((g) => ACTIVE_STATES.has(g.state));
        if (filter === "empty") groups = groups.filter((g) => g.state === "Empty");
        if (query.trim()) return fuzzyFilter(groups, query, (g) => g.groupId);
        const dir = sort.desc ? -1 : 1;
        return [...groups].sort((a, b) => {
            const av = a[sort.key] ?? -1;
            const bv = b[sort.key] ?? -1;
            return (av < bv ? -1 : av > bv ? 1 : a.groupId.localeCompare(b.groupId)) * dir;
        });
    });

    $effect(() => {
        void query;
        void filter;
        void sort;
        selected = visible.length ? 0 : -1;
    });

    $effect(() =>
        paletteCommands.set(
            "Consumer groups",
            all.map((g) => ({ id: `group:${g.groupId}`, label: g.groupId, detail: g.state, run: () => open(g) })),
        ),
    );

    function open(group: GroupSummary) {
        goto(`/group?id=${encodeURIComponent(group.groupId)}`);
    }

    function sortBy(key: SortKey) {
        sort = sort.key === key ? { key, desc: !sort.desc } : { key, desc: key === "totalLag" || key === "members" };
    }

    function onkeydown(event: KeyboardEvent) {
        const typing = event.target instanceof HTMLInputElement;
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

    const columns: { key: SortKey; label: string; align?: string }[] = [
        { key: "groupId", label: "Group" },
        { key: "state", label: "State" },
        { key: "members", label: "Members", align: "text-right" },
        { key: "totalLag", label: "Total lag", align: "text-right" },
    ];
    const grid = "grid grid-cols-[1fr_11rem_6rem_8rem_minmax(0,14rem)] gap-2 px-6";
</script>

<svelte:window {onkeydown} />

<main class="flex h-full flex-col">
    <header class="flex items-center gap-3 border-b px-6 py-3">
        <h1 class="text-lg font-semibold">Consumer Groups</h1>
        <div class="flex gap-1" role="radiogroup" aria-label="Filter groups">
            {#each [["all", "All"], ["active", "Active"], ["empty", "Empty"]] as [value, label] (value)}
                <button
                    role="radio"
                    aria-checked={filter === value}
                    onclick={() => (filter = value as Filter)}
                    class="rounded-md px-2 py-0.5 text-sm {filter === value ? 'bg-accent font-medium' : 'text-muted-foreground hover:bg-accent/50'}"
                >
                    {label} <span class="tabular-nums">{counts[value as Filter]}</span>
                </button>
            {/each}
        </div>
        <div class="ml-auto flex items-center gap-2">
            <span class="text-muted-foreground text-xs">{entry?.loading ? "Refreshing…" : `Updated ${ago(entry?.fetchedAt ?? null)}`}</span>
            <Button size="sm" variant="ghost" onclick={() => key && groupsCache.refresh(key)} aria-label="Refresh groups">
                <RefreshCw class="size-4 {entry?.loading ? 'animate-spin' : ''}" />
            </Button>
        </div>
    </header>

    <div class="border-b px-6 py-2">
        <input
            bind:this={search}
            bind:value={query}
            placeholder="Filter groups  ( / )"
            aria-label="Filter groups"
            class="bg-background h-8 w-full rounded-md border px-2 font-mono text-sm outline-none focus:ring-1"
        />
    </div>

    <div class="{grid} text-muted-foreground border-b py-1.5 text-xs font-medium">
        {#each columns as c (c.key)}
            <button class="{c.align ?? 'text-left'} hover:text-foreground" onclick={() => sortBy(c.key)}>
                {c.label}{sort.key === c.key ? (sort.desc ? " ↓" : " ↑") : ""}
            </button>
        {/each}
        <span>Topics</span>
    </div>

    <div class="min-h-0 flex-1">
        {#if !key}
            <p class="text-muted-foreground mt-16 text-center text-sm">No connection selected.</p>
        {:else if entry?.error && !entry.data}
            <ErrorState error={entry.error} onretry={() => groupsCache.refresh(key)} />
        {:else if !entry?.data}
            <SkeletonRows height={ROW} />
        {:else if visible.length === 0}
            <p class="text-muted-foreground mt-16 text-center text-sm">
                {all.length === 0 ? "No consumer groups on this cluster yet." : "No groups match."}
            </p>
        {:else}
            <VirtualList bind:this={list} items={visible} rowHeight={ROW} bind:selected onactivate={(g) => open(g)} class="h-full">
                {#snippet row(g: GroupSummary, _i: number, isSelected: boolean)}
                    <div class="{grid} h-full items-center text-sm {isSelected ? 'bg-accent' : 'hover:bg-accent/50'}">
                        <span class="truncate font-mono">{g.groupId}</span>
                        <span class={ACTIVE_STATES.has(g.state) ? "" : "text-muted-foreground"}>{g.state}</span>
                        <span class="text-right tabular-nums">{g.members}</span>
                        <span class="text-right font-mono tabular-nums {lagClass(g.totalLag)}">{formatLag(g.totalLag)}</span>
                        <span class="text-muted-foreground truncate font-mono text-xs" title={g.topics.join(", ")}>{g.topics.join(", ")}</span>
                    </div>
                {/snippet}
            </VirtualList>
        {/if}
    </div>
</main>

<script lang="ts">
    import { page } from "$app/state";
    import { goto } from "$app/navigation";
    import ArrowLeft from "@lucide/svelte/icons/arrow-left";
    import RefreshCw from "@lucide/svelte/icons/refresh-cw";
    import { Button } from "$lib/components/ui/button";
    import ErrorState from "$lib/components/app/error-state.svelte";
    import SkeletonRows from "$lib/components/app/skeleton-rows.svelte";
    import { ACTIVE_STATES, formatLag, lagClass } from "@/lag";
    import { ago } from "@/stores/cache.svelte";
    import { connections } from "@/stores/connections.svelte";
    import { groupDetailCache } from "@/stores/groups.svelte";

    const groupId = $derived(page.url.searchParams.get("id") ?? "");
    const key = $derived(connections.activeId && groupId ? `${connections.activeId}\u0000${groupId}` : null);
    const entry = $derived(key ? groupDetailCache.entry(key) : null);
    $effect(() => {
        if (key) groupDetailCache.ensure(key);
    });

    let onlyLagging = $state(false);
    const detail = $derived(entry?.data ?? null);
    const rows = $derived((detail?.partitions ?? []).filter((p) => !onlyLagging || (p.lag ?? 0) > 0));
    const fmt = (n: number | null) => (n === null ? "—" : n.toLocaleString());

    function onkeydown(event: KeyboardEvent) {
        if (event.key === "Escape" && !(event.target instanceof HTMLInputElement)) goto("/groups");
    }
</script>

<svelte:window {onkeydown} />

<main class="flex h-full flex-col">
    <header class="flex items-center gap-3 border-b px-6 py-3">
        <Button size="sm" variant="ghost" onclick={() => goto("/groups")} aria-label="Back to groups"><ArrowLeft class="size-4" /></Button>
        <h1 class="truncate font-mono text-lg font-semibold">{groupId}</h1>
        {#if detail}
            <span class="text-sm {ACTIVE_STATES.has(detail.state) ? '' : 'text-muted-foreground'}">{detail.state}</span>
            <span class="text-muted-foreground text-sm">· {detail.members.length} members ·</span>
            <span class="text-sm">lag <span class="font-mono tabular-nums {lagClass(detail.totalLag)}">{formatLag(detail.totalLag)}</span></span>
        {/if}
        <div class="ml-auto flex items-center gap-2">
            <span class="text-muted-foreground text-xs">{entry?.loading ? "Refreshing…" : `Updated ${ago(entry?.fetchedAt ?? null)}`}</span>
            <Button size="sm" variant="ghost" onclick={() => key && groupDetailCache.refresh(key)} aria-label="Refresh"><RefreshCw class="size-4" /></Button>
        </div>
    </header>

    <div class="min-h-0 flex-1 overflow-y-auto">
        {#if entry?.error && !detail}
            <ErrorState error={entry.error} onretry={() => key && groupDetailCache.refresh(key)} />
        {:else if !detail}
            <SkeletonRows />
        {:else}
            {#if detail.members.length}
                <section class="border-b px-6 py-3">
                    <h2 class="text-muted-foreground mb-1 text-xs font-medium">Members</h2>
                    <ul class="grid gap-0.5 text-sm">
                        {#each detail.members as m (m.memberId)}
                            <li class="flex gap-3 font-mono text-xs">
                                <span class="w-48 truncate" title={m.clientId}>{m.clientId}</span>
                                <span class="text-muted-foreground w-40 truncate">{m.host}</span>
                                <span class="text-muted-foreground truncate">
                                    {m.assignment.map((a) => `${a.topic}[${a.partitions.join(",")}]`).join("  ") || "no assignment"}
                                </span>
                            </li>
                        {/each}
                    </ul>
                </section>
            {/if}
            <div class="text-muted-foreground flex items-center gap-3 px-6 py-2 text-xs">
                <span>{detail.partitions.length} partitions</span>
                <label class="flex items-center gap-1.5"><input type="checkbox" bind:checked={onlyLagging} /> Only lagging</label>
            </div>
            <table class="w-full text-sm">
                <thead class="text-muted-foreground bg-background sticky top-0 border-y text-left text-xs">
                    <tr>
                        <th class="px-6 py-1.5 font-medium">Topic</th>
                        <th class="text-right font-medium">Partition</th>
                        <th class="text-right font-medium">Committed</th>
                        <th class="text-right font-medium">Log end</th>
                        <th class="text-right font-medium">Lag</th>
                        <th class="px-6 font-medium">Consumer</th>
                    </tr>
                </thead>
                <tbody class="divide-y font-mono tabular-nums">
                    {#each rows as p (p.topic + ":" + p.partition)}
                        <tr>
                            <td class="px-6 py-1.5">
                                <a class="hover:underline" href={`/topic?name=${encodeURIComponent(p.topic)}`}>{p.topic}</a>
                            </td>
                            <td class="text-right">{p.partition}</td>
                            <td class="text-right">{fmt(p.committed)}</td>
                            <td class="text-right">{fmt(p.logEnd)}</td>
                            <td class="text-right {lagClass(p.lag)}">{formatLag(p.lag)}</td>
                            <td class="text-muted-foreground px-6 text-xs">
                                {p.clientId ? `${p.clientId} · ${p.host}` : "unassigned"}
                            </td>
                        </tr>
                    {/each}
                </tbody>
            </table>
        {/if}
    </div>
</main>

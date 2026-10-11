<script lang="ts">
    import { page } from "$app/state";
    import { goto } from "$app/navigation";
    import ArrowLeft from "@lucide/svelte/icons/arrow-left";
    import RefreshCw from "@lucide/svelte/icons/refresh-cw";
    import { Button } from "$lib/components/ui/button";
    import ErrorState from "$lib/components/app/error-state.svelte";
    import SkeletonRows from "$lib/components/app/skeleton-rows.svelte";
    import CopyCli from "$lib/components/app/copy-cli.svelte";
    import { topicCommands } from "@/cli";
    import { commands, type TopicConfigEntry, type TopicConsumer, type TopicDetail } from "@/bindings";
    import { SwrCache, ago } from "@/stores/cache.svelte";
    import { connections } from "@/stores/connections.svelte";

    type Tab = "partitions" | "configs" | "consumers";

    // Keys are "<connection id>\u0000<topic>"; the backend uses the active connection.
    const topicName = (key: string) => key.split("\u0000")[1];
    const details = new SwrCache<TopicDetail>((k) => commands.describeTopic(topicName(k)), 15_000);
    const configs = new SwrCache<TopicConfigEntry[]>((k) => commands.topicConfigs(topicName(k)), 60_000);
    const consumers = new SwrCache<TopicConsumer[]>((k) => commands.topicConsumers(topicName(k)), 15_000);

    const name = $derived(page.url.searchParams.get("name") ?? "");
    const key = $derived(connections.activeId && name ? `${connections.activeId}\u0000${name}` : null);
    let tab = $state<Tab>("partitions");
    let showDefaults = $state(false);

    const detail = $derived(key ? details.entry(key) : null);
    const configEntry = $derived(key && tab === "configs" ? configs.entry(key) : null);
    const consumerEntry = $derived(key && tab === "consumers" ? consumers.entry(key) : null);

    $effect(() => {
        if (!key) return;
        details.ensure(key);
        if (tab === "configs") configs.ensure(key);
        if (tab === "consumers") consumers.ensure(key);
    });

    const overrides = $derived((configEntry?.data ?? []).filter((c) => c.origin === "topicOverride").length);
    const shownConfigs = $derived(
        (configEntry?.data ?? []).filter((c) => showDefaults || c.origin === "topicOverride" || !c.isDefault),
    );

    function refresh() {
        if (!key) return;
        details.refresh(key);
        if (tab === "configs") configs.refresh(key);
        if (tab === "consumers") consumers.refresh(key);
    }

    const originLabel: Record<string, string> = {
        topicOverride: "topic override",
        dynamicBroker: "broker (dynamic)",
        dynamicDefaultBroker: "cluster default (dynamic)",
        staticBroker: "broker config",
        default: "Kafka default",
        unknown: "unknown",
    };

    const fmt = (n: number) => n.toLocaleString();
    const tabs: { id: Tab; label: string }[] = [
        { id: "partitions", label: "Partitions" },
        { id: "configs", label: "Configs" },
        { id: "consumers", label: "Consumers" },
    ];

    function onkeydown(event: KeyboardEvent) {
        if (event.target instanceof HTMLInputElement) return;
        if (event.key === "Escape") goto("/topics");
        const index = Number(event.key) - 1;
        if (!event.metaKey && !event.ctrlKey && tabs[index]) tab = tabs[index].id;
    }
</script>

<svelte:window {onkeydown} />

<main class="flex h-full flex-col">
    <header class="flex items-center gap-3 border-b px-6 py-3">
        <Button size="sm" variant="ghost" onclick={() => goto("/topics")} aria-label="Back to topics">
            <ArrowLeft class="size-4" />
        </Button>
        <h1 class="truncate font-mono text-lg font-semibold">{name}</h1>
        {#if detail?.data}
            {@const s = detail.data.summary}
            <span class="text-muted-foreground text-sm tabular-nums">
                {s.partitions} partitions · RF {s.replicationFactor} · {fmt(detail.data.messageCount)} messages
            </span>
            {#if s.underReplicated}
                <span class="rounded bg-amber-500/15 px-1.5 text-xs font-medium text-amber-700 dark:text-amber-300">
                    {s.underReplicated} under-replicated
                </span>
            {/if}
        {/if}
        <div class="ml-auto flex items-center gap-2">
            <span class="text-muted-foreground text-xs">
                {detail?.loading ? "Refreshing…" : `Updated ${ago(detail?.fetchedAt ?? null)}`}
            </span>
            <Button size="sm" variant="ghost" onclick={refresh} aria-label="Refresh"><RefreshCw class="size-4" /></Button>
            {#if connections.active}<CopyCli commands={topicCommands(connections.active, name)} />{/if}
            <Button size="sm" variant="outline" onclick={() => goto(`/messages?topic=${encodeURIComponent(name)}`)}>
                Browse messages
            </Button>
            <Button size="sm" variant="outline" onclick={() => goto(`/produce?topic=${encodeURIComponent(name)}`)}>
                Produce…
            </Button>
        </div>
    </header>

    <div class="flex gap-1 border-b px-6" role="tablist">
        {#each tabs as t, i (t.id)}
            <button
                role="tab"
                aria-selected={tab === t.id}
                onclick={() => (tab = t.id)}
                class="-mb-px border-b-2 px-3 py-2 text-sm {tab === t.id
                    ? 'border-foreground font-medium'
                    : 'text-muted-foreground border-transparent hover:text-foreground'}"
            >
                {t.label}<span class="text-muted-foreground ml-1.5 font-mono text-[10px]">{i + 1}</span>
            </button>
        {/each}
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto">
        {#if tab === "partitions"}
            {#if detail?.error && !detail.data}
                <ErrorState error={detail.error} onretry={refresh} />
            {:else if !detail?.data}
                <SkeletonRows />
            {:else}
                <table class="w-full text-sm">
                    <thead class="text-muted-foreground bg-background sticky top-0 border-b text-left text-xs">
                        <tr>
                            <th class="px-6 py-1.5 font-medium">Partition</th>
                            <th class="font-medium">Leader</th>
                            <th class="font-medium">Replicas</th>
                            <th class="font-medium">In-sync</th>
                            <th class="text-right font-medium">Low</th>
                            <th class="text-right font-medium">High</th>
                            <th class="px-6 text-right font-medium">Messages</th>
                        </tr>
                    </thead>
                    <tbody class="divide-y font-mono tabular-nums">
                        {#each detail.data.partitions as p (p.id)}
                            {@const behind = p.isr.length < p.replicas.length}
                            <tr class={behind ? "bg-amber-500/5" : ""}>
                                <td class="px-6 py-1.5">{p.id}</td>
                                <td class={p.leader < 0 ? "text-destructive" : ""}>{p.leader < 0 ? "none" : p.leader}</td>
                                <td>{p.replicas.join(", ")}</td>
                                <td class={behind ? "text-amber-700 dark:text-amber-300" : ""}>{p.isr.join(", ")}</td>
                                <td class="text-right">{fmt(p.low)}</td>
                                <td class="text-right">{fmt(p.high)}</td>
                                <td class="px-6 text-right">{fmt(Math.max(0, p.high - p.low))}</td>
                            </tr>
                        {/each}
                    </tbody>
                </table>
            {/if}
        {:else if tab === "configs"}
            {#if configEntry?.error && !configEntry.data}
                <ErrorState error={configEntry.error} onretry={refresh} />
            {:else if !configEntry?.data}
                <SkeletonRows />
            {:else}
                <div class="text-muted-foreground flex items-center gap-3 px-6 py-2 text-xs">
                    <span>{overrides} topic override{overrides === 1 ? "" : "s"}</span>
                    <label class="flex items-center gap-1.5">
                        <input type="checkbox" bind:checked={showDefaults} /> Show Kafka defaults
                    </label>
                </div>
                <table class="w-full text-sm">
                    <thead class="text-muted-foreground border-y text-left text-xs">
                        <tr>
                            <th class="px-6 py-1.5 font-medium">Name</th>
                            <th class="font-medium">Value</th>
                            <th class="px-6 font-medium">Source</th>
                        </tr>
                    </thead>
                    <tbody class="divide-y">
                        {#each shownConfigs as c (c.name)}
                            {@const override = c.origin === "topicOverride"}
                            <tr class={override ? "bg-blue-500/5" : ""}>
                                <td class="px-6 py-1.5 font-mono {override ? 'font-medium' : ''}">{c.name}</td>
                                <td class="font-mono break-all">
                                    {#if c.sensitive}<span class="text-muted-foreground">hidden</span>{:else}{c.value ?? ""}{/if}
                                </td>
                                <td class="px-6 text-xs {override ? 'text-blue-700 dark:text-blue-300' : 'text-muted-foreground'}">
                                    {originLabel[c.origin]}
                                </td>
                            </tr>
                        {/each}
                    </tbody>
                </table>
            {/if}
        {:else}
            {#if consumerEntry?.error && !consumerEntry.data}
                <ErrorState error={consumerEntry.error} onretry={refresh} />
            {:else if !consumerEntry?.data}
                <SkeletonRows rows={3} />
            {:else if consumerEntry.data.length === 0}
                <p class="text-muted-foreground mt-12 text-center text-sm">No consumer group has read this topic.</p>
            {:else}
                <table class="w-full text-sm">
                    <thead class="text-muted-foreground border-b text-left text-xs">
                        <tr>
                            <th class="px-6 py-1.5 font-medium">Group</th>
                            <th class="font-medium">State</th>
                            <th class="text-right font-medium">Members</th>
                            <th class="px-6 text-right font-medium">Lag on this topic</th>
                        </tr>
                    </thead>
                    <tbody class="divide-y">
                        {#each consumerEntry.data as c (c.groupId)}
                            <tr
                                class="hover:bg-accent/50 cursor-pointer"
                                onclick={() => goto(`/group?id=${encodeURIComponent(c.groupId)}`)}
                            >
                                <td class="px-6 py-1.5 font-mono">{c.groupId}</td>
                                <td>{c.state}</td>
                                <td class="text-right tabular-nums">{c.members}</td>
                                <td class="px-6 text-right font-mono tabular-nums">{c.lag === null ? "—" : fmt(c.lag)}</td>
                            </tr>
                        {/each}
                    </tbody>
                </table>
            {/if}
        {/if}
    </div>
</main>

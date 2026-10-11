<script lang="ts">
    import { page } from "$app/state";
    import { Channel } from "@tauri-apps/api/core";
    import Play from "@lucide/svelte/icons/play";
    import Square from "@lucide/svelte/icons/square";
    import X from "@lucide/svelte/icons/x";
    import { Button } from "$lib/components/ui/button";
    import JsonTree from "$lib/components/app/json-tree.svelte";
    import VirtualList from "$lib/components/app/virtual-list.svelte";
    import { commands, type BrowseEvent, type BrowsedMessage, type PartitionRange, type StopReason } from "@/bindings";
    import { toAppError, toastError, type AppError } from "@/ipc-service";
    import { connections } from "@/stores/connections.svelte";
    import { topicsCache } from "@/stores/topics.svelte";
    import { fromBase64, hexDump, preview, tryJson, utf8 } from "@/util/bytes";
    import { toast } from "svelte-sonner";

    type StartKind = "latest" | "earliest" | "offset" | "timestamp";
    type View = "json" | "utf8" | "hex" | "raw";

    /** Results are capped; older rows drop off the front. */
    const MAX_ROWS = 50_000;
    const ROW = 28;

    let topic = $state(page.url.searchParams.get("topic") ?? "");
    let partitionsText = $state("");
    let startKind = $state<StartKind>("latest");
    let latestCount = $state(100);
    let startOffset = $state(0);
    let startTime = $state(new Date(Date.now() - 3600_000).toISOString().slice(0, 16));
    let limit = $state(1000);

    let messages = $state.raw<BrowsedMessage[]>([]);
    let dropped = $state(0);
    let ranges = $state.raw<PartitionRange[]>([]);
    let running = $state(false);
    let browseId = $state<string | null>(null);
    let done = $state<{ reason: StopReason; count: number; elapsedMs: number } | null>(null);
    let error = $state<AppError | null>(null);

    let selected = $state(-1);
    let detailOpen = $state(false);
    /** The view the user picked; JSON falls back to UTF-8 for non-JSON values. */
    let preferredView = $state<View>("json");

    const key = $derived(connections.activeId);
    const topicNames = $derived((key ? topicsCache.entry(key).data ?? [] : []).filter((t) => !t.internal).map((t) => t.name));
    $effect(() => {
        if (key) topicsCache.ensure(key);
    });

    const current = $derived(selected >= 0 ? messages[selected] : undefined);
    const keyText = $derived(utf8(fromBase64(current?.key)));
    const valueBytes = $derived(fromBase64(current?.value));
    const valueText = $derived(utf8(valueBytes));
    const valueJson = $derived(tryJson(valueText?.text));
    const view = $derived<View>(preferredView === "json" && !valueJson.ok ? "utf8" : preferredView);

    function parsePartitions(): number[] | null {
        const text = partitionsText.trim();
        if (!text || text.toLowerCase() === "all") return [];
        const parts = text.split(/[\s,]+/).filter(Boolean).map(Number);
        return parts.every((p) => Number.isInteger(p) && p >= 0) ? parts : null;
    }

    async function start(event?: Event) {
        event?.preventDefault();
        const partitions = parsePartitions();
        if (partitions === null) {
            error = { kind: "invalidInput", message: "Partitions must be a list like 0, 2, 5 — or empty for all." };
            return;
        }
        const startPosition =
            startKind === "latest" ? { kind: "latest" as const, count: latestCount }
            : startKind === "earliest" ? { kind: "earliest" as const }
            : startKind === "offset" ? { kind: "offset" as const, offset: startOffset }
            : { kind: "timestamp" as const, timestamp: new Date(startTime).getTime() };

        messages = [];
        dropped = 0;
        ranges = [];
        done = null;
        error = null;
        selected = -1;
        detailOpen = false;
        running = true;
        const id = crypto.randomUUID();
        browseId = id;

        const channel = new Channel<BrowseEvent>();
        channel.onmessage = (event) => {
            if (browseId !== id) return;
            if (event.kind === "started") ranges = event.ranges;
            else if (event.kind === "batch") {
                let next = messages.concat(event.messages);
                if (next.length > MAX_ROWS) {
                    const excess = next.length - MAX_ROWS;
                    next = next.slice(excess);
                    dropped += excess;
                    if (selected >= 0) selected = Math.max(-1, selected - excess);
                }
                messages = next;
            } else done = { reason: event.reason, count: event.count, elapsedMs: event.elapsedMs };
        };
        try {
            await commands.browseMessages(id, { topic: topic.trim(), partitions, start: startPosition, limit }, channel);
        } catch (e) {
            if (browseId === id) error = toAppError(e);
        } finally {
            if (browseId === id) running = false;
        }
    }

    function stop() {
        if (browseId) commands.cancelBrowse(browseId).catch(toastError);
    }

    function copyMessage(m: BrowsedMessage) {
        const text = utf8(fromBase64(m.value));
        const json = tryJson(text?.text);
        const out = {
            topic,
            partition: m.partition,
            offset: m.offset,
            timestamp: m.timestamp === null ? null : new Date(m.timestamp).toISOString(),
            key: utf8(fromBase64(m.key))?.text ?? null,
            value: json.ok ? json.value : (text?.text ?? null),
            headers: Object.fromEntries(m.headers.map((h) => [h.key, utf8(fromBase64(h.value))?.text ?? null])),
        };
        navigator.clipboard.writeText(JSON.stringify(out, null, 2)).then(
            () => toast.success("Message copied as JSON"),
            (e) => toastError(e),
        );
    }

    const stopText: Record<StopReason, string> = {
        caught: "reached the end of every partition",
        limit: "hit the limit",
        cancelled: "stopped",
        idle: "no new messages arrived",
    };
    const time = (ms: number | null) => (ms === null ? "—" : new Date(ms).toLocaleString());
    const size = (n: number) => (n < 1024 ? `${n} B` : n < 1048576 ? `${(n / 1024).toFixed(1)} KB` : `${(n / 1048576).toFixed(1)} MB`);

    function onkeydown(event: KeyboardEvent) {
        if (event.key === "Escape" && detailOpen && !(event.target instanceof HTMLInputElement)) detailOpen = false;
    }

    const grid = "grid grid-cols-[4rem_7rem_11rem_10rem_1fr_5rem] gap-2 px-4";
</script>

<svelte:window {onkeydown} />

<main class="flex h-full flex-col">
    <form onsubmit={start} class="flex flex-wrap items-end gap-2 border-b px-4 py-3">
        <label class="grid gap-1">
            <span class="text-muted-foreground text-xs">Topic</span>
            <input list="topic-names" bind:value={topic} required placeholder="orders"
                class="bg-background h-8 w-56 rounded-md border px-2 font-mono text-sm" />
            <datalist id="topic-names">{#each topicNames as name (name)}<option value={name}></option>{/each}</datalist>
        </label>
        <label class="grid gap-1">
            <span class="text-muted-foreground text-xs">Partitions</span>
            <input bind:value={partitionsText} placeholder="all" class="bg-background h-8 w-24 rounded-md border px-2 font-mono text-sm" />
        </label>
        <label class="grid gap-1">
            <span class="text-muted-foreground text-xs">Start</span>
            <select bind:value={startKind} class="bg-background h-8 rounded-md border px-2 text-sm">
                <option value="latest">Last N per partition</option>
                <option value="earliest">Earliest</option>
                <option value="offset">Offset</option>
                <option value="timestamp">Timestamp</option>
            </select>
        </label>
        {#if startKind === "latest"}
            <label class="grid gap-1"><span class="text-muted-foreground text-xs">N</span>
                <input type="number" min="1" bind:value={latestCount} class="bg-background h-8 w-20 rounded-md border px-2 text-sm tabular-nums" /></label>
        {:else if startKind === "offset"}
            <label class="grid gap-1"><span class="text-muted-foreground text-xs">Offset</span>
                <input type="number" min="0" bind:value={startOffset} class="bg-background h-8 w-28 rounded-md border px-2 font-mono text-sm" /></label>
        {:else if startKind === "timestamp"}
            <label class="grid gap-1"><span class="text-muted-foreground text-xs">From (local time)</span>
                <input type="datetime-local" bind:value={startTime} class="bg-background h-8 rounded-md border px-2 text-sm" /></label>
        {/if}
        <label class="grid gap-1"><span class="text-muted-foreground text-xs">Limit</span>
            <input type="number" min="1" max={MAX_ROWS} bind:value={limit} class="bg-background h-8 w-24 rounded-md border px-2 text-sm tabular-nums" /></label>
        {#if running}
            <Button type="button" size="sm" variant="outline" onclick={stop}><Square class="size-3.5" /> Stop</Button>
        {:else}
            <Button type="submit" size="sm" disabled={!key || !topic.trim()}><Play class="size-3.5" /> Read</Button>
        {/if}
        <span class="text-muted-foreground ml-auto self-center text-xs tabular-nums" role="status">
            {#if running}Reading… {messages.length.toLocaleString()} messages
            {:else if done}{done.count.toLocaleString()} messages in {done.elapsedMs} ms — {stopText[done.reason]}
            {/if}
            {#if dropped}· {dropped.toLocaleString()} oldest dropped (showing the newest {MAX_ROWS.toLocaleString()}){/if}
        </span>
    </form>

    {#if error}
        <div class="border-destructive/40 bg-destructive/5 mx-4 mt-3 rounded-md border p-3 text-sm" role="alert">
            <p class="text-destructive font-medium">{error.message}</p>
            {#if error.hint}<p class="text-muted-foreground mt-1">{error.hint}</p>{/if}
        </div>
    {/if}

    <div class="flex min-h-0 flex-1">
        <div class="flex min-w-0 flex-1 flex-col">
            <div class="{grid} text-muted-foreground border-b py-1.5 text-xs font-medium">
                <span class="text-right">Part.</span><span class="text-right">Offset</span><span>Timestamp</span><span>Key</span><span>Value</span><span class="text-right">Size</span>
            </div>
            <div class="min-h-0 flex-1">
                {#if messages.length === 0}
                    <p class="text-muted-foreground mt-16 text-center text-sm">
                        {#if running}Waiting for messages…
                        {:else if done}No messages in the selected range.
                        {:else}Pick a topic and press Read. Browsing never joins or creates a consumer group.{/if}
                    </p>
                {:else}
                    <VirtualList items={messages} rowHeight={ROW} bind:selected onactivate={() => (detailOpen = true)} class="h-full">
                        {#snippet row(m: BrowsedMessage, _i: number, isSelected: boolean)}
                            <div class="{grid} h-full items-center font-mono text-xs {isSelected ? 'bg-accent' : 'hover:bg-accent/50'}">
                                <span class="text-right tabular-nums">{m.partition}</span>
                                <span class="text-right tabular-nums">{m.offset}</span>
                                <span class="text-muted-foreground tabular-nums">{time(m.timestamp)}</span>
                                <span class="truncate">{preview(m.key, 60)}</span>
                                <span class="truncate">{m.value === null ? "∅ tombstone" : preview(m.value)}</span>
                                <span class="text-muted-foreground text-right tabular-nums">{size(m.valueSize)}</span>
                            </div>
                        {/snippet}
                    </VirtualList>
                {/if}
            </div>
        </div>

        {#if detailOpen && current}
            <aside class="flex w-[44%] min-w-80 flex-col border-l" aria-label="Message detail">
                <div class="flex items-center gap-2 border-b px-3 py-2 text-xs">
                    <span class="font-mono">p{current.partition} · offset {current.offset}</span>
                    <span class="text-muted-foreground">{time(current.timestamp)}{current.timestampType === "logAppend" ? " (broker time)" : ""}</span>
                    <div class="ml-auto flex gap-1">
                        <Button size="sm" variant="outline" onclick={() => copyMessage(current)}>Copy as JSON</Button>
                        <Button size="sm" variant="ghost" onclick={() => (detailOpen = false)} aria-label="Close detail"><X class="size-4" /></Button>
                    </div>
                </div>
                <div class="min-h-0 flex-1 overflow-y-auto p-3 text-sm">
                    <h3 class="text-muted-foreground text-xs font-medium">Key</h3>
                    <pre class="mt-1 font-mono text-xs break-all whitespace-pre-wrap">{current.key === null ? "null" : keyText?.text}</pre>

                    {#if current.headers.length}
                        <h3 class="text-muted-foreground mt-3 text-xs font-medium">Headers</h3>
                        <dl class="mt-1 grid grid-cols-[auto_1fr] gap-x-3 font-mono text-xs">
                            {#each current.headers as h, i (i)}
                                <dt class="text-muted-foreground">{h.key}</dt>
                                <dd class="break-all">{h.value === null ? "null" : utf8(fromBase64(h.value))?.text}</dd>
                            {/each}
                        </dl>
                    {/if}

                    <div class="mt-3 flex items-center gap-1">
                        <h3 class="text-muted-foreground mr-2 text-xs font-medium">Value{current.truncated ? " (first 1 MB)" : ""}</h3>
                        {#each [["json", "JSON"], ["utf8", "UTF-8"], ["hex", "Hex"], ["raw", "Raw"]] as [id, label] (id)}
                            <button
                                class="rounded px-1.5 py-0.5 text-xs {view === id ? 'bg-accent font-medium' : 'text-muted-foreground hover:bg-accent/50'}"
                                disabled={id === "json" && !valueJson.ok}
                                onclick={() => (preferredView = id as View)}>{label}</button>
                        {/each}
                    </div>
                    <div class="bg-muted/40 mt-1 rounded-md p-2" data-value-view={view}>
                        {#if current.value === null}
                            <p class="text-muted-foreground font-mono text-xs">null (tombstone)</p>
                        {:else if view === "json" && valueJson.ok}
                            <JsonTree value={valueJson.value} />
                        {:else if view === "utf8"}
                            {#if valueText && !valueText.valid}<p class="mb-1 text-xs text-amber-700 dark:text-amber-400">Not valid UTF-8; invalid bytes shown as �.</p>{/if}
                            <pre class="font-mono text-xs break-all whitespace-pre-wrap">{valueText?.text}</pre>
                        {:else if view === "hex"}
                            <pre class="font-mono text-[11px] leading-4 whitespace-pre">{hexDump(valueBytes)}</pre>
                        {:else}
                            <p class="text-muted-foreground mb-1 text-xs">Base64, exactly as received</p>
                            <pre class="font-mono text-xs break-all whitespace-pre-wrap">{current.value}</pre>
                        {/if}
                    </div>
                </div>
            </aside>
        {/if}
    </div>

    {#if ranges.length}
        <footer class="text-muted-foreground border-t px-4 py-1 font-mono text-[11px] tabular-nums">
            {ranges.map((r) => `p${r.partition} ${r.start}→${r.end}`).join("   ")}
        </footer>
    {/if}
</main>

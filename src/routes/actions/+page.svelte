<script lang="ts">
    import { onMount } from "svelte";
    import { Button } from "$lib/components/ui/button";
    import { commands, type ActionEntry, type Outcome } from "@/bindings";
    import { environmentStyle } from "@/environments";
    import { toastError } from "@/ipc-service";
    import { toast } from "svelte-sonner";

    let entries = $state<ActionEntry[]>([]);
    let loaded = $state(false);

    const outcomeStyle: Record<Outcome, string> = {
        ok: "text-emerald-700 dark:text-emerald-400",
        failed: "text-destructive",
        blocked: "text-amber-700 dark:text-amber-400",
        dryRun: "text-muted-foreground",
        abandoned: "text-muted-foreground",
    };
    const outcomeLabel: Record<Outcome, string> = {
        ok: "ok", failed: "failed", blocked: "blocked", dryRun: "dry run", abandoned: "abandoned",
    };

    async function load() {
        try {
            entries = await commands.listActions(500);
        } catch (e) {
            toastError(e);
        } finally {
            loaded = true;
        }
    }

    onMount(load);

    async function exportLog() {
        try {
            const path = await commands.exportActionLog();
            toast.success("Action log exported", { description: path });
        } catch (e) {
            toastError(e);
        }
    }

    async function copyJson() {
        await navigator.clipboard.writeText(entries.map((e) => JSON.stringify(e)).join("\n"));
        toast.success(`Copied ${entries.length} entries`);
    }

    const time = (iso: string) => new Date(iso).toLocaleString();
</script>

<main class="container py-8">
    <div class="flex items-center justify-between">
        <div>
            <h1 class="text-lg font-semibold">Action log</h1>
            <p class="text-muted-foreground text-sm">Every write and unlock, including blocked attempts. Stored only on this machine.</p>
        </div>
        <div class="flex gap-2">
            <Button size="sm" variant="ghost" onclick={load}>Refresh</Button>
            <Button size="sm" variant="outline" onclick={copyJson} disabled={!entries.length}>Copy as JSON</Button>
            <Button size="sm" variant="outline" onclick={exportLog}>Export file</Button>
        </div>
    </div>

    {#if loaded && entries.length === 0}
        <p class="text-muted-foreground mt-10 text-center text-sm">No actions yet. Writes you make or that get blocked will show up here.</p>
    {:else}
        <table class="mt-6 w-full text-sm">
            <thead class="text-muted-foreground border-b text-left text-xs">
                <tr>
                    <th class="py-2 font-medium">Time</th>
                    <th class="font-medium">Connection</th>
                    <th class="font-medium">Action</th>
                    <th class="font-medium">Target</th>
                    <th class="font-medium">Outcome</th>
                    <th class="font-medium">Detail</th>
                </tr>
            </thead>
            <tbody class="divide-y">
                {#each entries as entry (entry.id + entry.timestamp + entry.outcome)}
                    {@const env = environmentStyle(entry.environment)}
                    <tr>
                        <td class="text-muted-foreground py-1.5 whitespace-nowrap tabular-nums">{time(entry.timestamp)}</td>
                        <td class="whitespace-nowrap">
                            <span class="mr-1 inline-block h-2.5 w-1 rounded-sm {env.band}"></span>{entry.profileName}
                        </td>
                        <td>{entry.action === "unlockSession" ? "unlock" : entry.action}{entry.dryRun ? " (dry run)" : ""}</td>
                        <td class="font-mono">{entry.target}</td>
                        <td class="font-medium {outcomeStyle[entry.outcome]}">{outcomeLabel[entry.outcome]}</td>
                        <td class="text-muted-foreground max-w-80 truncate" title={entry.detail ?? ""}>{entry.detail ?? ""}</td>
                    </tr>
                {/each}
            </tbody>
        </table>
    {/if}
</main>

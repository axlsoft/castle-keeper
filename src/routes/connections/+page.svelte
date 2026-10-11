<script lang="ts">
    import { onMount } from "svelte";
    import { page } from "$app/state";
    import { replaceState } from "$app/navigation";
    import { Button } from "$lib/components/ui/button";
    import ConnectionDialog from "$lib/components/app/connection-dialog.svelte";
    import type { Profile } from "@/bindings";
    import { environmentStyle } from "@/environments";
    import { toastError } from "@/ipc-service";
    import { connections } from "@/stores/connections.svelte";

    let dialogOpen = $state(false);
    let editing = $state<Profile | null>(null);
    let confirmingDelete = $state<string | null>(null);

    onMount(() => {
        connections.refresh().catch(toastError);
    });

    // The command palette links here with ?new=1 to open the dialog directly.
    $effect(() => {
        if (page.url.searchParams.get("new") === "1") {
            add();
            replaceState("/connections", {});
        }
    });

    function add() {
        editing = null;
        dialogOpen = true;
    }

    function edit(profile: Profile) {
        editing = profile;
        dialogOpen = true;
    }

    async function remove(profile: Profile) {
        if (confirmingDelete !== profile.id) {
            confirmingDelete = profile.id;
            return;
        }
        confirmingDelete = null;
        await connections.remove(profile.id).catch(toastError);
    }

    function securityLabel(profile: Profile) {
        const sasl = profile.sasl
            ? { plain: "SASL/PLAIN", scramSha256: "SCRAM-SHA-256", scramSha512: "SCRAM-SHA-512" }[profile.sasl.mechanism]
            : null;
        const tls = profile.tls?.enabled ? "TLS" : null;
        return [sasl, tls].filter(Boolean).join(" + ") || "Plaintext";
    }
</script>

<main class="max-w-3xl p-6">
    <div class="flex items-center justify-between">
        <h1 class="text-lg font-semibold">Connections</h1>
        <Button size="sm" onclick={add}>New connection</Button>
    </div>

    {#if connections.loaded && connections.profiles.length === 0}
        <div class="mt-10 rounded-lg border border-dashed p-8 text-center">
            <p class="font-medium">No connections yet</p>
            <p class="text-muted-foreground mt-1 text-sm">
                Add a cluster to browse topics, consumer groups and messages.
            </p>
            <Button class="mt-4" onclick={add}>Add a connection</Button>
        </div>
    {:else}
        <ul class="mt-6 divide-y rounded-lg border">
            {#each connections.profiles as profile (profile.id)}
                {@const env = environmentStyle(profile.environment)}
                {@const active = profile.id === connections.activeId}
                <li class="flex items-center gap-3 py-3 pr-3">
                    <span class="h-10 w-1 rounded-r {env.band}" aria-hidden="true"></span>
                    <div class="min-w-0 flex-1">
                        <div class="flex items-center gap-2">
                            <span class="truncate font-medium">{profile.name}</span>
                            <span class="rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase {env.badge}">{env.label}</span>
                            {#if profile.readOnly}
                                <span class="text-muted-foreground text-[10px] font-semibold uppercase">Read-only</span>
                            {/if}
                            {#if active}
                                <span class="text-[10px] font-semibold uppercase text-emerald-600">Active</span>
                            {/if}
                        </div>
                        <p class="text-muted-foreground truncate font-mono text-xs">
                            {profile.bootstrap} · {securityLabel(profile)}
                        </p>
                    </div>
                    {#if !active}
                        <Button size="sm" variant="outline" onclick={() => connections.select(profile.id).catch(toastError)}>
                            Use
                        </Button>
                    {/if}
                    <Button size="sm" variant="ghost" onclick={() => edit(profile)}>Edit</Button>
                    <Button
                        size="sm"
                        variant={confirmingDelete === profile.id ? "destructive" : "ghost"}
                        onclick={() => remove(profile)}
                        onblur={() => (confirmingDelete = null)}
                    >
                        {confirmingDelete === profile.id ? "Confirm delete" : "Delete"}
                    </Button>
                </li>
            {/each}
        </ul>
    {/if}
</main>

<ConnectionDialog bind:open={dialogOpen} profile={editing} />

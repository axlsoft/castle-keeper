<script lang="ts">
    import { onMount } from "svelte";
    import { goto } from "$app/navigation";
    import * as DropdownMenu from "$lib/components/ui/dropdown-menu";
    import { environmentStyle } from "@/environments";
    import { toastError } from "@/ipc-service";
    import { connections } from "@/stores/connections.svelte";

    onMount(() => {
        connections.refresh().catch(toastError);
    });

    const active = $derived(connections.active);
</script>

<DropdownMenu.Root>
    <DropdownMenu.Trigger
        class="hover:bg-accent flex h-8 max-w-56 items-center gap-2 rounded-md border px-2 text-sm"
    >
        {#if active}
            {@const env = environmentStyle(active.environment)}
            <span class="h-3 w-1.5 rounded-sm {env.band}" aria-hidden="true"></span>
            <span class="truncate">{active.name}</span>
            <span class="rounded px-1 text-[10px] font-semibold uppercase {env.badge}">{env.label}</span>
        {:else}
            <span class="text-muted-foreground">No connection</span>
        {/if}
    </DropdownMenu.Trigger>
    <DropdownMenu.Content align="end" class="w-64">
        {#each connections.profiles as profile (profile.id)}
            {@const env = environmentStyle(profile.environment)}
            <DropdownMenu.Item onclick={() => connections.select(profile.id).catch(toastError)}>
                <span class="h-3 w-1.5 rounded-sm {env.band}" aria-hidden="true"></span>
                <span class="flex-1 truncate">{profile.name}</span>
                <span class="text-muted-foreground text-[10px] uppercase">{env.label}</span>
            </DropdownMenu.Item>
        {/each}
        {#if connections.profiles.length}
            <DropdownMenu.Separator />
        {/if}
        <DropdownMenu.Item onclick={() => goto("/connections")}>Manage connections…</DropdownMenu.Item>
    </DropdownMenu.Content>
</DropdownMenu.Root>

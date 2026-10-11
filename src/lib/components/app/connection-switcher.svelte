<script lang="ts">
    import { goto } from "$app/navigation";
    import ChevronsUpDown from "@lucide/svelte/icons/chevrons-up-down";
    import * as DropdownMenu from "$lib/components/ui/dropdown-menu";
    import { environmentStyle } from "@/environments";
    import { toastError } from "@/ipc-service";
    import { connections } from "@/stores/connections.svelte";

    const active = $derived(connections.active);
</script>

<DropdownMenu.Root>
    <DropdownMenu.Trigger
        class="hover:bg-accent flex w-full items-center gap-2 rounded-md border px-2 py-1.5 text-left text-sm"
        aria-label="Switch connection"
    >
        {#if active}
            {@const env = environmentStyle(active.environment)}
            <span class="h-8 w-1 shrink-0 rounded-sm {env.band}" aria-hidden="true"></span>
            <span class="min-w-0 flex-1">
                <span class="block truncate font-medium">{active.name}</span>
                <span class="text-muted-foreground block truncate text-[11px]">{env.label} · {active.bootstrap}</span>
            </span>
        {:else}
            <span class="text-muted-foreground flex-1">No connection</span>
        {/if}
        <ChevronsUpDown class="text-muted-foreground size-4 shrink-0" />
    </DropdownMenu.Trigger>
    <DropdownMenu.Content align="start" class="w-60">
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

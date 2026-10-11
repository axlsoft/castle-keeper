<script lang="ts">
    import { goto } from "$app/navigation";
    import * as Command from "$lib/components/ui/command";
    import Plug from "@lucide/svelte/icons/plug";
    import Plus from "@lucide/svelte/icons/plus";
    import { environmentStyle } from "@/environments";
    import { toastError } from "@/ipc-service";
    import { EXTRA_PAGES, NAV } from "@/navigation";
    import { shortcut } from "@/platform";
    import { connections } from "@/stores/connections.svelte";
    import { paletteCommands } from "@/stores/palette.svelte";

    let { open = $bindable(false) }: { open: boolean } = $props();

    function run(action: () => unknown) {
        open = false;
        Promise.resolve(action()).catch(toastError);
    }
</script>

<Command.Dialog bind:open label="Command palette">
    <Command.Input placeholder="Go to a page, switch connection, or run a command…" />
    <Command.List class="max-h-96">
        <Command.Empty>No matches.</Command.Empty>

        {#if paletteCommands.items.length}
            <Command.Group heading={paletteCommands.heading}>
                {#each paletteCommands.items as item (item.id)}
                    <Command.Item value={item.label} keywords={item.keywords} onSelect={() => run(item.run)}>
                        <span class="flex-1 truncate font-mono text-xs">{item.label}</span>
                        {#if item.detail}<span class="text-muted-foreground text-xs">{item.detail}</span>{/if}
                    </Command.Item>
                {/each}
            </Command.Group>
        {/if}

        <Command.Group heading="Go to">
            {#each [...NAV, ...EXTRA_PAGES] as page (page.href)}
                <Command.Item value={page.label} onSelect={() => run(() => goto(page.href))}>
                    <page.icon />
                    <span class="flex-1">{page.label}</span>
                    {#if page.key}<Command.Shortcut>{shortcut(page.key)}</Command.Shortcut>{/if}
                </Command.Item>
            {/each}
        </Command.Group>

        <Command.Group heading="Connections">
            {#each connections.profiles as profile (profile.id)}
                {@const env = environmentStyle(profile.environment)}
                <Command.Item
                    value={`Switch to ${profile.name}`}
                    keywords={[env.label, profile.bootstrap]}
                    onSelect={() => run(() => connections.select(profile.id))}
                >
                    <Plug />
                    <span class="flex-1">Switch to {profile.name}</span>
                    <span class="rounded px-1 text-[10px] font-semibold uppercase {env.badge}">{env.label}</span>
                </Command.Item>
            {/each}
            <Command.Item value="New connection" onSelect={() => run(() => goto("/connections?new=1"))}>
                <Plus />
                <span>New connection…</span>
            </Command.Item>
        </Command.Group>
    </Command.List>
</Command.Dialog>

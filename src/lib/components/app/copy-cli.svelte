<script lang="ts">
    import Terminal from "@lucide/svelte/icons/terminal";
    import * as DropdownMenu from "#lib/components/ui/dropdown-menu/index.js";
    import { PASSWORD_ENV, type CliCommand } from "#lib/cli.js";
    import { toastError } from "#lib/ipc-service.js";
    import { toast } from "svelte-sonner";

    let { commands, disabled = false }: { commands: CliCommand[]; disabled?: boolean } = $props();

    async function copy(command: CliCommand) {
        try {
            await navigator.clipboard.writeText(command.command);
            toast.success("Command copied", {
                description: command.command.includes(PASSWORD_ENV) ? `Set ${PASSWORD_ENV} before running it.` : undefined,
            });
        } catch (e) {
            toastError(e);
        }
    }
</script>

<DropdownMenu.Root>
    <DropdownMenu.Trigger
        {disabled}
        class="hover:bg-accent inline-flex h-8 items-center gap-1.5 rounded-md border px-2 text-sm disabled:opacity-50"
    >
        <Terminal class="size-4" /> Copy as CLI
    </DropdownMenu.Trigger>
    <DropdownMenu.Content align="end" class="w-72">
        {#each commands as command (command.label)}
            <DropdownMenu.Item onclick={() => copy(command)}>{command.label}</DropdownMenu.Item>
        {/each}
        <DropdownMenu.Separator />
        <p class="text-muted-foreground px-2 py-1 text-xs">Secrets are never included; passwords come from ${PASSWORD_ENV}.</p>
    </DropdownMenu.Content>
</DropdownMenu.Root>

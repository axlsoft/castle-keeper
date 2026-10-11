<script lang="ts">
    import * as Dialog from "$lib/components/ui/dialog";
    import { Button } from "$lib/components/ui/button";
    import Input from "$lib/components/ui/input/input.svelte";

    interface Props {
        open: boolean;
        title: string;
        description: string;
        /** The exact text the user must type, usually the connection name. */
        expected: string;
        confirmLabel: string;
        destructive?: boolean;
        onconfirm: (typed: string) => void | Promise<void>;
    }

    let {
        open = $bindable(),
        title,
        description,
        expected,
        confirmLabel,
        destructive = true,
        onconfirm,
    }: Props = $props();

    let typed = $state("");
    let busy = $state(false);

    $effect(() => {
        if (open) typed = "";
    });

    const matches = $derived(typed === expected);

    async function submit(event: Event) {
        event.preventDefault();
        if (!matches) return;
        busy = true;
        try {
            await onconfirm(typed);
            open = false;
        } finally {
            busy = false;
        }
    }
</script>

<Dialog.Root bind:open>
    <Dialog.Content class="max-w-md">
        <Dialog.Header>
            <Dialog.Title>{title}</Dialog.Title>
            <Dialog.Description>{description}</Dialog.Description>
        </Dialog.Header>
        <form onsubmit={submit} class="grid gap-3">
            <label class="grid gap-1 text-sm">
                <span>Type <code class="bg-muted rounded px-1 font-mono">{expected}</code> to confirm</span>
                <Input bind:value={typed} autocomplete="off" spellcheck={false} class="font-mono" />
            </label>
            <Dialog.Footer>
                <Button type="submit" variant={destructive ? "destructive" : "default"} disabled={!matches || busy}>
                    {confirmLabel}
                </Button>
            </Dialog.Footer>
        </form>
    </Dialog.Content>
</Dialog.Root>

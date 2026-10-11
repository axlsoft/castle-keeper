<script lang="ts">
    import { Button } from "#lib/components/ui/button/index.js";
    import type { AppError } from "#lib/ipc-service.js";

    let { error, onretry }: { error: AppError; onretry?: () => void } = $props();
</script>

<div class="mx-auto mt-10 max-w-md rounded-lg border p-6 text-center" role="alert">
    <p class="text-destructive font-medium">{error.message}</p>
    {#if error.hint}<p class="text-muted-foreground mt-1 text-sm">{error.hint}</p>{/if}
    {#if onretry}<Button class="mt-4" size="sm" variant="outline" onclick={onretry}>Retry</Button>{/if}
</div>

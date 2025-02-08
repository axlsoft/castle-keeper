<script lang="ts">
    import { page } from "$app/state";
    import { cn } from "$lib/utils.js";
</script>

<div class="mr-4 hidden md:flex">
    <a href="/" class="mr-6 flex items-center space-x-2">
        <span class="hidden font-bold xl:inline-block p-2">
            Castle Keeper
        </span>
    </a>
    <nav class="flex items-center gap-6 text-sm">
        <a
            href="/clusters"
            class={cn(
                "hover:text-foreground/80 transition-colors",
                page.url.pathname === "/clusters"
                    ? "text-foreground"
                    : "text-foreground/60",
            )}
        >
            Clusters
        </a>
    </nav>
</div>

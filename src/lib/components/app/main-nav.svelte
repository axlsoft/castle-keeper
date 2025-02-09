<script lang="ts">
    import { page } from "$app/state";
    import { cn } from "$lib/utils.js";
    import logo from "./icons/logo.png";
</script>

<div class="mr-4 hidden md:flex">
    <a href="/" class="mr-6 flex items-center space-x-2">
        <img src={logo} alt="Castle Keeper" class="size-6" />
        <span class="hidden font-bold xl:inline-block p-2">
            Castle Keeper
        </span>
    </a>
    <nav class="flex items-center gap-6 text-sm">
        <a
            href="/data"
            class={cn(
                "hover:text-foreground/80 transition-colors",
                page.url.pathname === "/data"
                    ? "text-foreground"
                    : "text-foreground/60",
            )}
        >
            Data Explorer
        </a>
        <a
            href="/clusters"
            class={cn(
                "hover:text-foreground/80 transition-colors",
                page.url.pathname === "/clusters"
                    ? "text-foreground"
                    : "text-foreground/60",
            )}
        >
            Cluster Management
        </a>
        <a
            href="/monitoring"
            class={cn(
                "hover:text-foreground/80 transition-colors",
                page.url.pathname === "/monitoring"
                    ? "text-foreground"
                    : "text-foreground/60",
            )}
        >
            Monitoring
        </a>
        <a
            href="/schemas"
            class={cn(
                "hover:text-foreground/80 transition-colors",
                page.url.pathname === "/schemas"
                    ? "text-foreground"
                    : "text-foreground/60",
            )}
        >
            Schemas
        </a>
    </nav>
</div>

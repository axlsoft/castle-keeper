<script lang="ts">
    import { page } from "$app/state";
    import Search from "@lucide/svelte/icons/search";
    import ConnectionSwitcher from "./connection-switcher.svelte";
    import ModeToggle from "./mode-toggle.svelte";
    import { NAV } from "@/navigation";
    import { shortcut } from "@/platform";
    import { cn } from "$lib/utils.js";
    import logo from "./icons/logo.png";

    let { onsearch }: { onsearch: () => void } = $props();

    const isActive = (href: string) => page.url.pathname === href || page.url.pathname.startsWith(`${href}/`);
</script>

<aside class="bg-muted/30 flex w-56 shrink-0 flex-col gap-3 border-r p-3">
    <div class="flex items-center gap-2 px-1">
        <img src={logo} alt="" class="size-5" />
        <span class="text-sm font-semibold">Castle Keeper</span>
    </div>

    <ConnectionSwitcher />

    <button
        type="button"
        onclick={onsearch}
        class="text-muted-foreground hover:bg-accent flex items-center gap-2 rounded-md border px-2 py-1.5 text-sm"
    >
        <Search class="size-4" />
        <span class="flex-1 text-left">Search…</span>
        <kbd class="bg-muted rounded px-1 font-mono text-[10px]">{shortcut("K")}</kbd>
    </button>

    <nav class="flex flex-col gap-0.5" aria-label="Main">
        {#each NAV as item (item.href)}
            <a
                href={item.href}
                aria-current={isActive(item.href) ? "page" : undefined}
                class={cn(
                    "group flex items-center gap-2 rounded-md px-2 py-1.5 text-sm transition-colors",
                    isActive(item.href) ? "bg-accent text-foreground font-medium" : "text-foreground/70 hover:bg-accent/60",
                )}
            >
                <item.icon class="size-4" />
                <span class="flex-1">{item.label}</span>
                {#if item.key}
                    <kbd class="text-muted-foreground hidden font-mono text-[10px] group-hover:inline">{shortcut(item.key)}</kbd>
                {/if}
            </a>
        {/each}
    </nav>

    <div class="mt-auto flex items-center justify-between px-1">
        <span class="text-muted-foreground text-[11px]">v0.1.0</span>
        <ModeToggle />
    </div>
</aside>

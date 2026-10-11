<script lang="ts" generics="T">
    import type { Snippet } from "svelte";

    interface Props {
        items: T[];
        rowHeight: number;
        /** Index of the keyboard-selected row, or -1. */
        selected?: number;
        overscan?: number;
        row: Snippet<[T, number, boolean]>;
        onactivate?: (item: T, index: number) => void;
        class?: string;
    }

    let {
        items,
        rowHeight,
        selected = $bindable(-1),
        overscan = 8,
        row,
        onactivate,
        class: className = "",
    }: Props = $props();

    let viewport: HTMLDivElement | undefined = $state();
    let scrollTop = $state(0);
    let height = $state(0);

    const start = $derived(Math.max(0, Math.floor(scrollTop / rowHeight) - overscan));
    const end = $derived(Math.min(items.length, Math.ceil((scrollTop + height) / rowHeight) + overscan));
    const visible = $derived(items.slice(start, end));

    function scrollIntoView(index: number) {
        if (!viewport) return;
        const top = index * rowHeight;
        if (top < viewport.scrollTop) viewport.scrollTop = top;
        else if (top + rowHeight > viewport.scrollTop + height) viewport.scrollTop = top + rowHeight - height;
    }

    function onkeydown(event: KeyboardEvent) {
        if (!items.length) return;
        const page = Math.max(1, Math.floor(height / rowHeight) - 1);
        let next = selected;
        switch (event.key) {
            case "ArrowDown": next = Math.min(items.length - 1, selected + 1); break;
            case "ArrowUp": next = Math.max(0, selected - 1); break;
            case "PageDown": next = Math.min(items.length - 1, selected + page); break;
            case "PageUp": next = Math.max(0, selected - page); break;
            case "Home": next = 0; break;
            case "End": next = items.length - 1; break;
            case "Enter":
                if (selected >= 0 && onactivate) onactivate(items[selected], selected);
                event.preventDefault();
                return;
            default: return;
        }
        event.preventDefault();
        selected = next;
        scrollIntoView(next);
    }

    /** Lets a parent move focus into the list (e.g. after a search box). */
    export function focus() {
        viewport?.focus();
        if (selected < 0 && items.length) selected = 0;
    }
</script>

<div
    bind:this={viewport}
    bind:clientHeight={height}
    onscroll={() => (scrollTop = viewport?.scrollTop ?? 0)}
    {onkeydown}
    tabindex="0"
    role="listbox"
    class="relative overflow-y-auto outline-none {className}"
>
    <div style="height: {items.length * rowHeight}px" class="relative">
        <div style="transform: translateY({start * rowHeight}px)" class="absolute inset-x-0 top-0">
            {#each visible as item, i (start + i)}
                <div
                    role="option"
                    aria-selected={start + i === selected}
                    tabindex="-1"
                    style="height: {rowHeight}px"
                    onclick={() => { selected = start + i; onactivate?.(item, start + i); }}
                    onkeydown={() => {}}
                >
                    {@render row(item, start + i, start + i === selected)}
                </div>
            {/each}
        </div>
    </div>
</div>

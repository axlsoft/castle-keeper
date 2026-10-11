<script lang="ts">
    import { resetMode, setMode, userPrefersMode } from "mode-watcher";

    function onchange(event: Event) {
        const value = (event.currentTarget as HTMLSelectElement).value;
        if (value === "light" || value === "dark") setMode(value);
        else resetMode();
    }
</script>

<main class="container py-8">
    <h1 class="text-lg font-semibold">Settings</h1>
    <label class="mt-4 flex max-w-xs flex-col gap-1 text-sm">
        <span class="font-medium">Theme</span>
        <select class="border-input bg-background rounded-md border px-2 py-1.5" value={userPrefersMode.current} {onchange}>
            <option value="system">System</option>
            <option value="light">Light</option>
            <option value="dark">Dark</option>
        </select>
    </label>
</main>

<script lang="ts">
    import type { PageData } from "./$types";

    import { onMount } from "svelte";
    import { theme, store } from "../../lib/stores/themeStore";

    let themeValue = $state("");

    async function saveSettings(e: Event) {
        e.preventDefault();

        theme.set(themeValue);
    }

    onMount(async () => {
        themeValue = (await store.get("theme")) || "light";
    });

    let { data }: { data: PageData } = $props();
</script>

<main>
    <h1>Settings</h1>
    <form onsubmit={saveSettings}>
        <label for="theme">Theme:</label>
        <select id="theme" bind:value={themeValue}>
            <option value="light">Light</option>
            <option value="dark">Dark</option>
        </select>
        <button type="submit">Save</button>
    </form>
</main>

<style>
    main {
        padding: 0px;
    }
    form {
        display: flex;
        flex-direction: column;
        gap: 1rem;
    }
    label {
        font-weight: bold;
    }
    select,
    button {
        padding: 0.5rem;
        font-size: 1rem;
    }
</style>

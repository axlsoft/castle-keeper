<script lang="ts">
	import "../app.css";
	import { onMount } from "svelte";
	import { goto } from "$app/navigation";
	import { Toaster } from "$lib/components/ui/sonner/index.js";
	import * as Tooltip from "$lib/components/ui/tooltip/index.js";
	import { ModeWatcher } from "mode-watcher";
	import AppSidebar from "$lib/components/app/app-sidebar.svelte";
	import CommandPalette from "$lib/components/app/command-palette.svelte";
	import EnvStrip from "$lib/components/app/env-strip.svelte";
	import { toastError } from "@/ipc-service";
	import { NAV } from "@/navigation";
	import { modalsSettled } from "@/modal";
	import { hasModifier } from "@/platform";
	import { connections } from "@/stores/connections.svelte";

	let { children } = $props();
	let paletteOpen = $state(false);

	onMount(() => {
		connections.refresh().catch(toastError);
	});

	function onkeydown(event: KeyboardEvent) {
		if (!hasModifier(event)) return;
		const key = event.key.toLowerCase();
		if (key === "k") {
			event.preventDefault();
			if (paletteOpen) paletteOpen = false;
			else modalsSettled().then(() => (paletteOpen = true));
			return;
		}
		const target = NAV.find((item) => item.key === key);
		if (target) {
			event.preventDefault();
			goto(target.href);
		}
	}
</script>

<svelte:window {onkeydown} />

<ModeWatcher />
<Toaster position="bottom-right" />

<Tooltip.Provider>
	<div class="bg-background flex h-screen flex-col overflow-hidden">
		<EnvStrip />
		<div class="flex min-h-0 flex-1">
			<AppSidebar onsearch={() => modalsSettled().then(() => (paletteOpen = true))} />
			<div class="min-w-0 flex-1 overflow-y-auto" id="page">
				{@render children?.()}
			</div>
		</div>
	</div>
</Tooltip.Provider>

<CommandPalette bind:open={paletteOpen} />

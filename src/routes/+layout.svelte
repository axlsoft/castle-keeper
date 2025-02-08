<script lang="ts">
	import "../app.pcss";
	import { Toaster } from "$lib/components/ui/sonner/index.js";
	import * as Tooltip from "$lib/components/ui/tooltip/index.js";
	import { ModeWatcher } from "mode-watcher";
	import AppHeader from "@/components/app/app-header.svelte";

	let { children } = $props();
</script>

<ModeWatcher />
<Toaster />

<AppHeader />

<Tooltip.Provider>
	<div
		class="bg-background relative flex min-h-screen flex-col"
		id="page"
		data-vaul-drawer-wrapper
	>
		{@render children?.()}
	</div>
</Tooltip.Provider>

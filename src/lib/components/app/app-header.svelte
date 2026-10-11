<script lang="ts">
	import ModeToggle from "./mode-toggle.svelte";
	import MainNav from "./main-nav.svelte";
	import ConnectionSwitcher from "./connection-switcher.svelte";
</script>

<header
	class="border-border/40 bg-background/95 supports-[backdrop-filter]:bg-background/60 sticky top-0 z-50 w-full border-b backdrop-blur"
>
	<div class="container flex h-14 max-w-screen-2xl items-center">
		<MainNav />
		<div
			class="flex flex-1 items-center justify-between space-x-2 md:justify-end"
		>
			<nav class="flex items-center gap-2">
				<ConnectionSwitcher />
				<ModeToggle />
			</nav>
		</div>
	</div>
</header>

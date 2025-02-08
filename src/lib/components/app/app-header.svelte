<script lang="ts">
	import ModeToggle from "./mode-toggle.svelte";
	import MainNav from "./main-nav.svelte";
	import { cn } from "$lib/utils.js";
	import { buttonVariants } from "../ui/button";
</script>

<header
	class="border-border/40 bg-background/95 supports-[backdrop-filter]:bg-background/60 sticky top-0 z-50 w-full border-b backdrop-blur"
>
	<div class="container flex h-14 max-w-screen-2xl items-center">
		<MainNav />
		<div
			class="flex flex-1 items-center justify-between space-x-2 md:justify-end"
		>
			<div class="w-full flex-1 md:w-auto md:flex-none">
				<!-- <CommandMenu /> -->
			</div>
			<nav class="flex items-center">
				<a href="#" target="_blank" rel="noopener noreferrer">
					<div
						class={cn(
							buttonVariants({
								size: "sm",
								variant: "ghost",
							}),
							"h-8 w-8 px-0",
						)}
					>
						<!-- <Icon.GitHub class="size-4" /> -->
						<span class="sr-only">GitHub</span>
					</div>
				</a>
				<a href="#" target="_blank" rel="noreferrer">
					<div
						class={cn(
							buttonVariants({
								size: "sm",
								variant: "ghost",
							}),
							"h-8 w-8 px-0",
						)}
					>
						<!-- <Icon.Twitter class="size-3 fill-current" /> -->
						<span class="sr-only"
							>X (formerly known as Twitter)</span
						>
					</div>
				</a>
				<ModeToggle />
			</nav>
		</div>
	</div>
</header>

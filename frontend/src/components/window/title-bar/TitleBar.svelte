<script lang="ts" context="module">
	export type Platform = "Windows" | "Mac" | "Linux" | "Web";
</script>

<script lang="ts">
	import LayoutRow from "@/components/layout/LayoutRow.svelte";
	import MenuBarInput from "@/components/widgets/inputs/MenuBarInput.svelte";
	import WindowButtonsMac from "@/components/window/title-bar/WindowButtonsMac.svelte";
	import WindowButtonsWeb from "@/components/window/title-bar/WindowButtonsWeb.svelte";
	import WindowButtonsWindows from "@/components/window/title-bar/WindowButtonsWindows.svelte";
	import WindowTitle from "@/components/window/title-bar/WindowTitle.svelte";
	import type { PortfolioState } from "@/state-providers/portfolio";
	import { getContext } from "svelte";

	export let platform: Platform;
	export let maximized: boolean;

	const portfolio = getContext<PortfolioState>("portfolio");

	$: docIndex = $portfolio.activeDocumentIndex;
	$: displayName = $portfolio.documents[docIndex]?.displayName || "";
	$: windowTitle = `${displayName}${displayName && " - "}Graphite`;
</script>

<LayoutRow class="title-bar">
	<LayoutRow class="header-part">
		{#if platform === "Mac"}
			<WindowButtonsMac {maximized} />
		{:else}
			<MenuBarInput />
		{/if}
	</LayoutRow>
	<LayoutRow class="header-part">
		<WindowTitle text={windowTitle} />
	</LayoutRow>
	<LayoutRow class="header-part">
		{#if platform === "Windows" || platform === "Linux"}
			<WindowButtonsWindows {maximized} />
		{:else if platform === "Web"}
			<WindowButtonsWeb />
		{/if}
	</LayoutRow>
</LayoutRow>

<style lang="scss" global>
	.title-bar {
		height: 28px;
		flex: 0 0 auto;

		.header-part {
			flex: 1 1 100%;

			&:nth-child(1) {
				justify-content: flex-start;
			}

			&:nth-child(2) {
				justify-content: center;
			}

			&:nth-child(3) {
				justify-content: flex-end;
			}
		}
	}
</style>

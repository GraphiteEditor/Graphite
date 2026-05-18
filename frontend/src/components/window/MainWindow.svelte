<script lang="ts">
	import { getContext } from "svelte";
	import Dialog from "/src/components/floating-menus/Dialog.svelte";
	import Tooltip from "/src/components/floating-menus/Tooltip.svelte";
	import LayoutCol from "/src/components/layout/LayoutCol.svelte";
	import LayoutRow from "/src/components/layout/LayoutRow.svelte";
	import TextLabel from "/src/components/widgets/labels/TextLabel.svelte";
	import PanelSubdivision from "/src/components/window/PanelSubdivision.svelte";
	import StatusBar from "/src/components/window/StatusBar.svelte";
	import TitleBar from "/src/components/window/TitleBar.svelte";
	import type { AppWindowStore } from "/src/stores/app-window";
	import type { DialogStore } from "/src/stores/dialog";
	import type { PortfolioStore } from "/src/stores/portfolio";
	import type { TooltipStore } from "/src/stores/tooltip";

	const dialog = getContext<DialogStore>("dialog");
	const tooltip = getContext<TooltipStore>("tooltip");
	const appWindow = getContext<AppWindowStore>("appWindow");
	const portfolio = getContext<PortfolioStore>("portfolio");
</script>

<LayoutCol class="main-window" classes={{ "viewport-hole-punch": $appWindow.viewportHolePunch }}>
	{#if !($appWindow.platform == "Mac" && $appWindow.fullscreen)}
		<TitleBar />
	{/if}
	<LayoutRow class="workspace" data-workspace>
		<PanelSubdivision subdivision={$portfolio.panelLayout.root} depth={0} />
	</LayoutRow>
	<StatusBar />
	{#if $dialog.visible}
		<Dialog />
	{/if}
	{#if $tooltip.visible}
		<Tooltip />
	{/if}
	{#if import.meta.env.MODE === "native" && new Date() > new Date("2026-07-01")}
		<LayoutCol class="release-candidate-expiry">
			<TextLabel>
				<p>
					This is an outdated desktop release candidate build. Its testing<br />
					period has concluded and the next build is available for download.<br />
					Please update to help us continue testing by reporting new issues.
				</p>
			</TextLabel>
		</LayoutCol>
	{/if}
</LayoutCol>

<style lang="scss">
	.main-window {
		height: 100%;
		overflow: auto;
		touch-action: none;
	}

	.release-candidate-expiry {
		position: absolute;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -50%);
		background-color: var(--color-e-nearwhite);
		color: var(--color-2-mildblack);
		opacity: 0.9;
		pointer-events: none;
		padding: 12px 40px;
		border-radius: 4px;
		text-align-last: justify;
		font-size: 18px;
		z-index: 1000;

		.text-label {
			line-height: 1.5;
		}
	}
</style>

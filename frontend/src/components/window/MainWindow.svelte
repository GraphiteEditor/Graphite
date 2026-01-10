<script lang="ts">
	import { getContext } from "svelte";

	import type { AppWindowState } from "@graphite/state-providers/app-window";
	import type { DialogState } from "@graphite/state-providers/dialog";
	import type { TooltipState } from "@graphite/state-providers/tooltip";
	import { isDesktop } from "@graphite/utility-functions/platform";

	import Dialog from "@graphite/components/floating-menus/Dialog.svelte";
	import Tooltip from "@graphite/components/floating-menus/Tooltip.svelte";
	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";
	import StatusBar from "@graphite/components/window/status-bar/StatusBar.svelte";
	import TitleBar from "@graphite/components/window/title-bar/TitleBar.svelte";
	import Workspace from "@graphite/components/window/workspace/Workspace.svelte";

	const dialog = getContext<DialogState>("dialog");
	const tooltip = getContext<TooltipState>("tooltip");
	const appWindow = getContext<AppWindowState>("appWindow");
</script>

<LayoutCol class="main-window" classes={{ "viewport-hole-punch": $appWindow.viewportHolePunch }}>
	{#if !($appWindow.platform == "Mac" && $appWindow.fullscreen)}
		<TitleBar />
	{/if}
	<Workspace />
	<StatusBar />
	{#if $dialog.visible}
		<Dialog />
	{/if}
	{#if $tooltip.visible}
		<Tooltip />
	{/if}
	{#if isDesktop() && new Date() > new Date("2026-01-31")}
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

<style lang="scss" global>
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

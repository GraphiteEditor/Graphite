<script lang="ts">
	import { getContext } from "svelte";

	import type { AppWindowState } from "@graphite/state-providers/app-window";
	import type { DialogState } from "@graphite/state-providers/dialog";
	import type { TooltipState } from "@graphite/state-providers/tooltip";

	import Dialog from "@graphite/components/floating-menus/Dialog.svelte";
	import Tooltip from "@graphite/components/floating-menus/Tooltip.svelte";
	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import StatusBar from "@graphite/components/window/status-bar/StatusBar.svelte";
	import TitleBar from "@graphite/components/window/title-bar/TitleBar.svelte";
	import Workspace from "@graphite/components/window/workspace/Workspace.svelte";

	const dialog = getContext<DialogState>("dialog");
	const tooltip = getContext<TooltipState>("tooltip");
	const appWindow = getContext<AppWindowState>("appWindow");
</script>

<LayoutCol class="main-window" classes={{ "viewport-hole-punch": $appWindow.viewportHolePunch }}>
	<TitleBar />
	<Workspace />
	<StatusBar />
	{#if $dialog.visible}
		<Dialog />
	{/if}
	{#if $tooltip.visible}
		<Tooltip />
	{/if}
</LayoutCol>

<style lang="scss" global>
	.main-window {
		height: 100%;
		overflow: auto;
		touch-action: none;
	}
</style>

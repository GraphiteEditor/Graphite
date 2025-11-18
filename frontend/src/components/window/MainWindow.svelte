<script lang="ts">
	import { getContext } from "svelte";

	import type { AppWindowState } from "@graphite/state-providers/app-window";

	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import StatusBar from "@graphite/components/window/status-bar/StatusBar.svelte";
	import TitleBar from "@graphite/components/window/title-bar/TitleBar.svelte";
	import Workspace from "@graphite/components/window/workspace/Workspace.svelte";

	const appWindow = getContext<AppWindowState>("appWindow");
</script>

<LayoutCol class="main-window" classes={{ "viewport-hole-punch": $appWindow.viewportHolePunch }}>
	<TitleBar />

	<Workspace />

	<StatusBar />
</LayoutCol>

<style lang="scss" global>
	.main-window {
		height: 100%;
		overflow: auto;
		touch-action: none;
	}
</style>

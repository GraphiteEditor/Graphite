<script lang="ts">
	import type { AppWindowPlatform } from "@graphite/messages";

	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import StatusBar from "@graphite/components/window/status-bar/StatusBar.svelte";
	import TitleBar from "@graphite/components/window/title-bar/TitleBar.svelte";
	import Workspace from "@graphite/components/window/workspace/Workspace.svelte";

	export let platform: AppWindowPlatform;
	export let maximized: boolean;
	export let viewportHolePunch: boolean;
</script>

<LayoutCol class="main-window" classes={{ "viewport-hole-punch": viewportHolePunch }}>
	<TitleBar {platform} {maximized} />

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

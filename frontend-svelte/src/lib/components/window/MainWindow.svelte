<script lang="ts" context="module">
	export type ApplicationPlatform = "Windows" | "Mac" | "Linux" | "Web";
</script>

<script lang="ts">
	import LayoutCol from "$lib/components/layout/LayoutCol.svelte";
	import StatusBar from "$lib/components/window/status-bar/StatusBar.svelte";
	import TitleBar from "$lib/components/window/title-bar/TitleBar.svelte";
	import Workspace from "$lib/components/window/workspace/Workspace.svelte";

	let platform: ApplicationPlatform = "Web";
	let maximized: true;
</script>

<LayoutCol class="main-window">
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

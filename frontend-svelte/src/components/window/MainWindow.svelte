<script lang="ts" context="module">
	export type ApplicationPlatform = "Windows" | "Mac" | "Linux" | "Web";
</script>

<script lang="ts">
	import LayoutCol from "@/components/layout/LayoutCol.svelte";
	import StatusBar from "@/components/window/status-bar/StatusBar.svelte";
	import TitleBar from "@/components/window/title-bar/TitleBar.svelte";
	import Workspace from "@/components/window/workspace/Workspace.svelte";

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

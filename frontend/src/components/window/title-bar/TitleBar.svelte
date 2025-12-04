<script lang="ts">
	import { getContext, onMount } from "svelte";

	import type { Editor } from "@graphite/editor";
	import type { Layout } from "@graphite/messages";
	import { patchLayout, UpdateMenuBarLayout } from "@graphite/messages";
	import type { AppWindowState } from "@graphite/state-providers/app-window";

	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import WidgetLayout from "@graphite/components/widgets/WidgetLayout.svelte";
	import WindowButtonsLinux from "@graphite/components/window/title-bar/WindowButtonsLinux.svelte";
	import WindowButtonsWeb from "@graphite/components/window/title-bar/WindowButtonsWeb.svelte";
	import WindowButtonsWindows from "@graphite/components/window/title-bar/WindowButtonsWindows.svelte";

	const appWindow = getContext<AppWindowState>("appWindow");
	const editor = getContext<Editor>("editor");

	let menuBarLayout: Layout = [];

	onMount(() => {
		editor.subscriptions.subscribeJsMessage(UpdateMenuBarLayout, (updateMenuBarLayout) => {
			patchLayout(menuBarLayout, updateMenuBarLayout);
			menuBarLayout = menuBarLayout;
		});
	});
</script>

<LayoutRow class="title-bar">
	<!-- Menu bar -->
	<LayoutRow>
		{#if $appWindow.platform !== "Mac"}
			<WidgetLayout layout={menuBarLayout} layoutTarget="MenuBar" />
		{/if}
	</LayoutRow>
	<!-- Spacer -->
	<LayoutRow class="spacer" on:mousedown={() => editor.handle.appWindowDrag()} on:dblclick={() => editor.handle.appWindowMaximize()} />
	<!-- Window buttons -->
	<LayoutRow>
		{#if $appWindow.platform === "Web"}
			<WindowButtonsWeb />
		{:else if $appWindow.platform === "Windows"}
			<WindowButtonsWindows />
		{:else if $appWindow.platform === "Linux"}
			<WindowButtonsLinux />
		{/if}
	</LayoutRow>
</LayoutRow>

<style lang="scss" global>
	.title-bar {
		height: 28px;
		flex: 0 0 auto;

		> .layout-row {
			flex: 0 0 auto;

			> .widget-span {
				--row-height: 28px;

				> * {
					--widget-height: 28px;
				}
			}

			&.spacer {
				flex: 1 1 100%;
			}

			.text-button {
				height: 100%;
			}
		}
	}
</style>

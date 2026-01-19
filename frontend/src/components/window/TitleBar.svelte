<script lang="ts">
	import { getContext, onMount } from "svelte";

	import type { Editor } from "@graphite/editor";
	import type { Layout } from "@graphite/messages";
	import { patchLayout, UpdateMenuBarLayout } from "@graphite/messages";
	import type { AppWindowState } from "@graphite/state-providers/app-window";
	import type { FullscreenState } from "@graphite/state-providers/fullscreen";
	import type { TooltipState } from "@graphite/state-providers/tooltip";

	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";
	import WidgetLayout from "@graphite/components/widgets/WidgetLayout.svelte";
	import { isDesktop } from "/src/utility-functions/platform";

	const appWindow = getContext<AppWindowState>("appWindow");
	const editor = getContext<Editor>("editor");
	const fullscreen = getContext<FullscreenState>("fullscreen");
	const tooltip = getContext<TooltipState>("tooltip");

	let menuBarLayout: Layout = [];

	$: showFullscreenButton = $appWindow.platform === "Web" || $fullscreen.windowFullscreen || (isDesktop() && $appWindow.fullscreen);
	$: isFullscreen = isDesktop() ? $appWindow.fullscreen : $fullscreen.windowFullscreen;
	// On Mac, the menu bar height needs to be scaled by the inverse of the UI scale to fit its native window buttons
	$: height = $appWindow.platform === "Mac" ? 28 * (1 / $appWindow.uiScale) : 28;

	onMount(() => {
		editor.subscriptions.subscribeJsMessage(UpdateMenuBarLayout, (data) => {
			patchLayout(menuBarLayout, data);
			menuBarLayout = menuBarLayout;
		});
	});
</script>

<LayoutRow class="title-bar" styles={{ height: height + "px" }}>
	<!-- Menu bar -->
	<LayoutRow class="menu-bar">
		{#if $appWindow.platform !== "Mac"}
			<WidgetLayout layout={menuBarLayout} layoutTarget="MenuBar" />
		{/if}
	</LayoutRow>
	<!-- Window frame -->
	<LayoutRow class="window-frame" on:mousedown={() => !isFullscreen && editor.handle.appWindowDrag()} on:dblclick={() => !isFullscreen && editor.handle.appWindowMaximize()} />
	<!-- Window buttons -->
	<LayoutRow class="window-buttons" classes={{ fullscreen: showFullscreenButton, windows: $appWindow.platform === "Windows", linux: $appWindow.platform === "Linux" }}>
		{#if $appWindow.platform !== "Mac"}
			{#if showFullscreenButton}
				<LayoutRow
					tooltipLabel={isFullscreen ? "Exit Fullscreen" : "Enter Fullscreen"}
					tooltipDescription={$appWindow.platform === "Web" && $fullscreen.keyboardLockApiSupported
						? "While fullscreen, keyboard shortcuts normally reserved by the browser become available."
						: undefined}
					tooltipShortcut={$tooltip.fullscreenShortcut}
					on:click={() => {
						if (isDesktop()) editor.handle.appWindowFullscreen();
						else ($fullscreen.windowFullscreen ? fullscreen.exitFullscreen : fullscreen.enterFullscreen)();
					}}
				>
					<IconLabel icon={isFullscreen ? "FullscreenExit" : "FullscreenEnter"} />
				</LayoutRow>
			{:else}
				<LayoutRow tooltipLabel="Minimize" on:click={() => editor.handle.appWindowMinimize()}>
					<IconLabel icon="WindowButtonWinMinimize" />
				</LayoutRow>
				<LayoutRow tooltipLabel={$appWindow.maximized ? ($appWindow.platform === "Windows" ? "Restore Down" : "Unmaximize") : "Maximize"} on:click={() => editor.handle.appWindowMaximize()}>
					<IconLabel icon={$appWindow.maximized ? "WindowButtonWinRestoreDown" : "WindowButtonWinMaximize"} />
				</LayoutRow>
				<LayoutRow tooltipLabel="Close" on:click={() => editor.handle.appWindowClose()}>
					<IconLabel icon="WindowButtonWinClose" />
				</LayoutRow>
			{/if}
		{/if}
	</LayoutRow>
</LayoutRow>

<style lang="scss" global>
	.title-bar {
		flex: 0 0 auto;

		> .layout-row {
			flex: 0 0 auto;

			> .widget-span {
				--row-height: 28px;

				> * {
					--widget-height: 28px;
				}
			}

			&.window-frame {
				flex: 1 1 100%;
			}

			.text-button {
				height: 100%;
			}
		}

		.window-buttons {
			> .layout-row {
				flex: 0 0 auto;
				align-items: center;

				svg {
					fill: var(--color-e-nearwhite);
				}

				&:hover {
					background: var(--color-6-lowergray);

					svg {
						fill: var(--color-f-white);
					}
				}
			}

			&.fullscreen > .layout-row {
				padding: 0 8px;
			}

			&.windows:not(.fullscreen) > .layout-row {
				padding: 0 17px;

				&:hover {
					background: #2d2d2d;
				}

				&:last-of-type:hover {
					background: #c42b1c;
				}
			}

			&.linux:not(.fullscreen) > .layout-row {
				padding: 0 12px;

				&:hover {
					border-radius: 2px;
				}
			}
		}
	}

	// paddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpadding
</style>

<script lang="ts">
	import { getContext, onMount, onDestroy } from "svelte";
	import LayoutRow from "/src/components/layout/LayoutRow.svelte";
	import IconLabel from "/src/components/widgets/labels/IconLabel.svelte";
	import WidgetLayout from "/src/components/widgets/WidgetLayout.svelte";
	import type { AppWindowStore } from "/src/stores/app-window";
	import { enterFullscreen, exitFullscreen } from "/src/stores/fullscreen";
	import type { FullscreenStore } from "/src/stores/fullscreen";
	import type { TooltipStore } from "/src/stores/tooltip";
	import type { SubscriptionsRouter } from "/src/subscriptions-router";
	import { patchLayout } from "/src/utility-functions/widgets";
	import type { EditorWrapper, Layout } from "/wrapper/pkg/graphite_wasm_wrapper";
	import { isPlatformNative } from "/wrapper/pkg/graphite_wasm_wrapper";

	const keyboardLockApiSupported = navigator.keyboard !== undefined && "lock" in navigator.keyboard;

	const editor = getContext<EditorWrapper>("editor");
	const subscriptions = getContext<SubscriptionsRouter>("subscriptions");
	const appWindow = getContext<AppWindowStore>("appWindow");
	const fullscreen = getContext<FullscreenStore>("fullscreen");
	const tooltip = getContext<TooltipStore>("tooltip");

	let menuBarLayout: Layout = [];

	$: showFullscreenButton = $appWindow.platform === "Web" || $fullscreen.windowFullscreen || (isPlatformNative() && $appWindow.fullscreen);
	$: isFullscreen = isPlatformNative() ? $appWindow.fullscreen : $fullscreen.windowFullscreen;
	// On Mac, the menu bar height needs to be scaled by the inverse of the UI scale to fit its native window buttons
	$: height = $appWindow.platform === "Mac" ? 28 * (1 / $appWindow.uiScale) : 28;

	onMount(() => {
		subscriptions.subscribeLayoutUpdate("MenuBar", (data) => {
			patchLayout(menuBarLayout, data);
			menuBarLayout = menuBarLayout;
		});
	});

	onDestroy(() => {
		subscriptions.unsubscribeLayoutUpdate("MenuBar");
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
	<LayoutRow class="window-frame" on:mousedown={() => !isFullscreen && editor.appWindowDrag()} on:dblclick={() => !isFullscreen && editor.appWindowMaximize()} />
	<!-- Window buttons -->
	<LayoutRow class="window-buttons" classes={{ fullscreen: showFullscreenButton, windows: $appWindow.platform === "Windows", linux: $appWindow.platform === "Linux" }}>
		{#if $appWindow.platform !== "Mac"}
			{#if showFullscreenButton}
				<LayoutRow
					tooltipLabel={isFullscreen ? "Exit Fullscreen" : "Enter Fullscreen"}
					tooltipDescription={$appWindow.platform === "Web" && keyboardLockApiSupported
						? "While fullscreen, keyboard shortcuts normally reserved by the browser become available."
						: undefined}
					tooltipShortcut={$tooltip.fullscreenShortcut}
					on:click={() => {
						if (isPlatformNative()) editor.appWindowFullscreen();
						else ($fullscreen.windowFullscreen ? exitFullscreen : enterFullscreen)();
					}}
				>
					<IconLabel icon={isFullscreen ? "FullscreenExit" : "FullscreenEnter"} />
				</LayoutRow>
			{:else}
				<LayoutRow tooltipLabel="Minimize" on:click={() => editor.appWindowMinimize()}>
					<IconLabel icon="WindowButtonWinMinimize" />
				</LayoutRow>
				<LayoutRow tooltipLabel={$appWindow.maximized ? ($appWindow.platform === "Windows" ? "Restore Down" : "Unmaximize") : "Maximize"} on:click={() => editor.appWindowMaximize()}>
					<IconLabel icon={$appWindow.maximized ? "WindowButtonWinRestoreDown" : "WindowButtonWinMaximize"} />
				</LayoutRow>
				<LayoutRow tooltipLabel="Close" on:click={() => editor.appWindowClose()}>
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

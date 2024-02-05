<script lang="ts" context="module">
	export type Platform = "Windows" | "Mac" | "Linux" | "Web";
</script>

<script lang="ts">
	import { getContext, onMount } from "svelte";

	import type { PortfolioState } from "@graphite/state-providers/portfolio";
	import { platformIsMac } from "@graphite/utility-functions/platform";
	import type { Editor } from "@graphite/wasm-communication/editor";
	import { type KeyRaw, type LayoutKeysGroup, type MenuBarEntry, type MenuListEntry, UpdateMenuBarLayout } from "@graphite/wasm-communication/messages";

	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import TextButton from "@graphite/components/widgets/buttons/TextButton.svelte";
	import WindowButtonsMac from "@graphite/components/window/title-bar/WindowButtonsMac.svelte";
	import WindowButtonsWeb from "@graphite/components/window/title-bar/WindowButtonsWeb.svelte";
	import WindowButtonsWindows from "@graphite/components/window/title-bar/WindowButtonsWindows.svelte";
	import WindowTitle from "@graphite/components/window/title-bar/WindowTitle.svelte";

	export let platform: Platform;
	export let maximized: boolean;

	const editor = getContext<Editor>("editor");
	const portfolio = getContext<PortfolioState>("portfolio");

	// TODO: Apparently, Safari does not support the Keyboard.lock() API but does relax its authority over certain keyboard shortcuts in fullscreen mode, which we should take advantage of
	const ACCEL_KEY = platformIsMac() ? "Command" : "Control";
	const LOCK_REQUIRING_SHORTCUTS: KeyRaw[][] = [
		[ACCEL_KEY, "KeyW"],
		[ACCEL_KEY, "KeyN"],
		[ACCEL_KEY, "Shift", "KeyN"],
		[ACCEL_KEY, "KeyT"],
		[ACCEL_KEY, "Shift", "KeyT"],
	];

	let entries: MenuListEntry[] = [];

	$: docIndex = $portfolio.activeDocumentIndex;
	$: displayName = $portfolio.documents[docIndex]?.displayName || "";
	$: windowTitle = `${displayName}${displayName && " - "}Graphite`;

	onMount(() => {
		const arraysEqual = (a: KeyRaw[], b: KeyRaw[]): boolean => a.length === b.length && a.every((aValue, i) => aValue === b[i]);
		const shortcutRequiresLock = (shortcut: LayoutKeysGroup): boolean => {
			const shortcutKeys = shortcut.map((keyWithLabel) => keyWithLabel.key);

			// If this shortcut matches any of the browser-reserved shortcuts
			return LOCK_REQUIRING_SHORTCUTS.some((lockKeyCombo) => arraysEqual(shortcutKeys, lockKeyCombo));
		};

		editor.subscriptions.subscribeJsMessage(UpdateMenuBarLayout, (updateMenuBarLayout) => {
			const menuBarEntryToMenuListEntry = (entry: MenuBarEntry): MenuListEntry => ({
				// From `MenuEntryCommon`
				...entry,

				// Shared names with fields that need to be converted from the type used in `MenuBarEntry` to that of `MenuListEntry`
				action: () => editor.instance.widgetValueCommitAndUpdate(updateMenuBarLayout.layoutTarget, entry.action.widgetId, undefined),
				children: entry.children ? entry.children.map((entries) => entries.map((entry) => menuBarEntryToMenuListEntry(entry))) : undefined,

				// New fields in `MenuListEntry`
				shortcutRequiresLock: entry.shortcut ? shortcutRequiresLock(entry.shortcut.keys) : undefined,
				value: undefined,
				disabled: entry.disabled ?? undefined,
				font: undefined,
				ref: undefined,
			});

			entries = updateMenuBarLayout.layout.map(menuBarEntryToMenuListEntry);
		});
	});
</script>

<LayoutRow class="title-bar">
	<!-- Menu bar (or on Mac: window buttons) -->
	<LayoutRow class="left">
		{#if platform === "Mac"}
			<WindowButtonsMac {maximized} />
		{:else}
			{#each entries as entry}
				<TextButton label={entry.label} icon={entry.icon} menuListChildren={entry.children} action={entry.action} flush={true} />
			{/each}
		{/if}
	</LayoutRow>
	<!-- Document title -->
	<LayoutRow class="center">
		<WindowTitle text={windowTitle} />
	</LayoutRow>
	<!-- Window buttons (except on Mac) -->
	<LayoutRow class="right">
		{#if platform === "Windows" || platform === "Linux"}
			<WindowButtonsWindows {maximized} />
		{:else if platform === "Web"}
			<WindowButtonsWeb />
		{/if}
	</LayoutRow>
</LayoutRow>

<style lang="scss" global>
	.title-bar {
		height: 28px;
		flex: 0 0 auto;

		> .layout-row {
			flex: 1 1 100%;

			&.left {
				justify-content: flex-start;
			}

			&.center {
				justify-content: center;
			}

			&.right {
				justify-content: flex-end;
			}
		}

		.text-button {
			height: 28px;
		}
	}
</style>

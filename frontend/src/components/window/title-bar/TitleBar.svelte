<script lang="ts">
	import { getContext, onMount } from "svelte";

	import type { Editor } from "@graphite/editor";
	import { type KeyRaw, type LayoutKeysGroup, type MenuBarEntry, type MenuListEntry, type AppWindowPlatform, UpdateMenuBarLayout } from "@graphite/messages";
	import { platformIsMac } from "@graphite/utility-functions/platform";

	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import TextButton from "@graphite/components/widgets/buttons/TextButton.svelte";
	import WindowButtonsLinux from "@graphite/components/window/title-bar/WindowButtonsLinux.svelte";
	import WindowButtonsWeb from "@graphite/components/window/title-bar/WindowButtonsWeb.svelte";
	import WindowButtonsWindows from "@graphite/components/window/title-bar/WindowButtonsWindows.svelte";

	export let platform: AppWindowPlatform;
	export let maximized: boolean;

	const editor = getContext<Editor>("editor");

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
				action: () => editor.handle.widgetValueCommitAndUpdate(updateMenuBarLayout.layoutTarget, entry.action.widgetId, undefined),
				children: entry.children ? entry.children.map((entries) => entries.map((entry) => menuBarEntryToMenuListEntry(entry))) : undefined,

				// New fields in `MenuListEntry`
				shortcutRequiresLock: entry.shortcut ? shortcutRequiresLock(entry.shortcut.keys) : undefined,
				value: "",
				disabled: entry.disabled ?? undefined,
				font: undefined,
			});

			entries = updateMenuBarLayout.layout.map(menuBarEntryToMenuListEntry);
		});
	});
</script>

<LayoutRow class="title-bar">
	<!-- Menu bar -->
	<LayoutRow>
		{#if platform !== "Mac"}
			{#each entries as entry}
				<TextButton label={entry.label} icon={entry.icon} menuListChildren={entry.children} action={entry.action} flush={true} />
			{/each}
		{/if}
	</LayoutRow>
	<!-- Spacer -->
	<LayoutRow class="spacer" on:mousedown={() => editor.handle.appWindowDrag()} on:dblclick={() => editor.handle.appWindowMaximize()} />
	<!-- Window buttons -->
	<LayoutRow>
		{#if platform === "Web"}
			<WindowButtonsWeb />
		{:else if platform === "Windows"}
			<WindowButtonsWindows {maximized} />
		{:else if platform === "Linux"}
			<WindowButtonsLinux {maximized} />
		{/if}
	</LayoutRow>
</LayoutRow>

<style lang="scss" global>
	.title-bar {
		height: 28px;
		flex: 0 0 auto;

		> .layout-row {
			flex: 0 0 auto;

			&.spacer {
				flex: 1 1 100%;
			}

			.text-button {
				height: 100%;
			}
		}
	}
</style>

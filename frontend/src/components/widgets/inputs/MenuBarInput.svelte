<script lang="ts">
	import { getContext, onMount } from "svelte";

	import { platformIsMac } from "@graphite/utility-functions/platform";
	import type { Editor } from "@graphite/wasm-communication/editor";
	import { type KeyRaw, type LayoutKeysGroup, type MenuBarEntry, type MenuListEntry, UpdateMenuBarLayout } from "@graphite/wasm-communication/messages";

	import MenuList from "@graphite/components/floating-menus/MenuList.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";

	// TODO: Apparently, Safari does not support the Keyboard.lock() API but does relax its authority over certain keyboard shortcuts in fullscreen mode, which we should take advantage of
	const accelKey = platformIsMac() ? "Command" : "Control";
	const LOCK_REQUIRING_SHORTCUTS: KeyRaw[][] = [
		[accelKey, "KeyW"],
		[accelKey, "KeyN"],
		[accelKey, "Shift", "KeyN"],
		[accelKey, "KeyT"],
		[accelKey, "Shift", "KeyT"],
	];

	const editor = getContext<Editor>("editor");

	let entries: MenuListEntry[] = [];

	function clickEntry(menuListEntry: MenuListEntry, e: MouseEvent) {
		// If there's no menu to open, trigger the action but don't try to open its non-existant children
		if (!menuListEntry.children || menuListEntry.children.length === 0) {
			if (menuListEntry.action && !menuListEntry.disabled) menuListEntry.action();

			return;
		}

		// Focus the target so that keyboard inputs are sent to the dropdown
		(e.target as HTMLElement | undefined)?.focus();

		if (menuListEntry.ref) {
			menuListEntry.ref.open = true;
			entries = entries;
		} else {
			throw new Error("The menu bar floating menu has no associated ref");
		}
	}

	onMount(() => {
		editor.subscriptions.subscribeJsMessage(UpdateMenuBarLayout, (updateMenuBarLayout) => {
			const arraysEqual = (a: KeyRaw[], b: KeyRaw[]): boolean => a.length === b.length && a.every((aValue, i) => aValue === b[i]);
			const shortcutRequiresLock = (shortcut: LayoutKeysGroup): boolean => {
				const shortcutKeys = shortcut.map((keyWithLabel) => keyWithLabel.key);

				// If this shortcut matches any of the browser-reserved shortcuts
				return LOCK_REQUIRING_SHORTCUTS.some((lockKeyCombo) => arraysEqual(shortcutKeys, lockKeyCombo));
			};

			const menuBarEntryToMenuListEntry = (entry: MenuBarEntry): MenuListEntry => ({
				// From `MenuEntryCommon`
				...entry,

				// Shared names with fields that need to be converted from the type used in `MenuBarEntry` to that of `MenuListEntry`
				action: () => editor.instance.updateLayout(updateMenuBarLayout.layoutTarget, entry.action.widgetId, undefined),
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

<div class="menu-bar-input" data-menu-bar-input>
	{#each entries as entry}
		<div class="entry-container">
			<!-- svelte-ignore a11y-no-noninteractive-tabindex -->
			<div
				on:click={(e) => clickEntry(entry, e)}
				on:keydown={(e) => entry.ref?.keydown(e, false)}
				class="entry"
				class:open={entry.ref?.open}
				tabindex="0"
				data-floating-menu-spawner={entry.children && entry.children.length > 0 ? "" : "no-hover-transfer"}
			>
				{#if entry.icon}
					<IconLabel icon={entry.icon} />
				{/if}
				{#if entry.label}
					<TextLabel>{entry.label}</TextLabel>
				{/if}
			</div>
			{#if entry.children && entry.children.length > 0}
				<MenuList
					on:open={({ detail }) => {
						if (entry.ref) {
							entry.ref.open = detail;
							entries = entries;
						}
					}}
					open={entry.ref?.open || false}
					entries={entry.children || []}
					direction="Bottom"
					minWidth={240}
					drawIcon={true}
					bind:this={entry.ref}
				/>
			{/if}
		</div>
	{/each}
</div>

<style lang="scss" global>
	.menu-bar-input {
		display: flex;

		.entry-container {
			display: flex;
			position: relative;

			.entry {
				display: flex;
				align-items: center;
				white-space: nowrap;
				padding: 0 8px;
				background: none;
				border: 0;
				margin: 0;
				border-radius: 2px;

				&:hover,
				&.open {
					background: var(--color-5-dullgray);
				}
			}
		}
	}
</style>

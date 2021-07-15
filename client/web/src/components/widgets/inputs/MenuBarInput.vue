<template>
	<div class="menu-bar-input">
		<div class="entry-container">
			<div @click="handleLogoClick(entry)" class="entry">
				<IconLabel :icon="'GraphiteLogo'" />
			</div>
		</div>
		<div class="entry-container" v-for="entry in menuEntries" :key="entry">
			<div @click="handleEntryClick(entry)" class="entry" :class="{ open: entry.ref && entry.ref.isOpen() }" data-hover-menu-spawner>
				<IconLabel :icon="entry.icon" v-if="entry.icon" />
				<span v-if="entry.label">{{ entry.label }}</span>
			</div>
			<MenuList :menuEntries="entry.children" :direction="MenuDirection.Bottom" :minWidth="240" :drawIcon="true" :defaultAction="actionNotImplemented" :ref="(ref) => setEntryRefs(entry, ref)" />
		</div>
	</div>
</template>

<style lang="scss">
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

			svg {
				fill: var(--color-e-nearwhite);
			}

			&:hover,
			&.open {
				background: var(--color-6-lowergray);

				svg {
					fill: var(--color-f-white);
				}

				span {
					color: var(--color-f-white);
				}
			}
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";
import { ApplicationPlatform } from "@/components/window/MainWindow.vue";
import MenuList, { MenuListEntry, MenuListEntries } from "@/components/widgets/floating-menus/MenuList.vue";
import { MenuDirection } from "@/components/widgets/floating-menus/FloatingMenu.vue";

const wasm = import("@/../wasm/pkg");

const menuEntries: MenuListEntries = [
	{
		label: "File",
		ref: undefined,
		children: [
			[
				{ label: "New", icon: "File", shortcut: ["Ctrl", "N"], shortcutRequiresLock: true, action: async () => (await wasm).new_document() },
				{ label: "Open…", shortcut: ["Ctrl", "O"] },
				{
					label: "Open Recent",
					shortcut: ["Ctrl", "⇧", "O"],
					children: [
						[{ label: "Reopen Last Closed", shortcut: ["Ctrl", "⇧", "T"], shortcutRequiresLock: true }, { label: "Clear Recently Opened" }],
						[
							{ label: "Some Recent File.gdd" },
							{ label: "Another Recent File.gdd" },
							{ label: "An Older File.gdd" },
							{ label: "Some Other Older File.gdd" },
							{ label: "Yet Another Older File.gdd" },
						],
					],
				},
			],
			[
				{ label: "Close", shortcut: ["Ctrl", "W"], shortcutRequiresLock: true, action: async () => (await wasm).close_active_document_with_confirmation() },
				{ label: "Close All", shortcut: ["Ctrl", "Alt", "W"], action: async () => (await wasm).close_all_documents_with_confirmation() },
			],
			[
				{ label: "Save", shortcut: ["Ctrl", "S"] },
				{ label: "Save As…", shortcut: ["Ctrl", "⇧", "S"] },
				{ label: "Save All", shortcut: ["Ctrl", "Alt", "S"] },
				{ label: "Auto-Save", shortcut: undefined },
			],
			[
				{ label: "Import…", shortcut: ["Ctrl", "I"] },
				{ label: "Export…", shortcut: ["Ctrl", "E"], action: async () => (await wasm).export_document() },
			],
			[{ label: "Quit", shortcut: ["Ctrl", "Q"] }],
		],
	},
	{
		label: "Edit",
		ref: undefined,
		children: [
			[
				{ label: "Undo", shortcut: ["Ctrl", "Z"], action: async () => (await wasm).undo() },
				{ label: "Redo", shortcut: ["Ctrl", "⇧", "Z"] },
			],
			[
				{ label: "Cut", shortcut: ["Ctrl", "X"] },
				{ label: "Copy", icon: "Copy", shortcut: ["Ctrl", "C"] },
				{ label: "Paste", icon: "Paste", shortcut: ["Ctrl", "V"] },
			],
		],
	},
	{
		label: "Document",
		ref: undefined,
		children: [[{ label: "Menu not yet populated" }]],
	},
	{
		label: "Layer",
		ref: undefined,
		children: [
			[
				{ label: "Select All", shortcut: ["Ctrl", "A"], action: async () => (await wasm).select_all_layers() },
				{ label: "Deselect All", shortcut: ["Ctrl", "Alt", "A"], action: async () => (await wasm).deselect_all_layers() },
				{ label: "Move Layer Up", shortcut: ["Ctrl", "]"], action: async () => (await wasm).move_selected_layer(1) },
				{ label: "Move Layer Down", shortcut: ["Ctrl", "["], action: async () => (await wasm).move_selected_layer(-1) },
				{ label: "Move Layer To Top", shortcut: ["Ctrl", "Shift", "]"], action: async () => (await wasm).move_selected_layer(999999) },
				{ label: "Move Layer To Bottom", shortcut: ["Ctrl", "Shift", "["], action: async () => (await wasm).move_selected_layer(-999999) },
			],
		],
	},
	{
		label: "View",
		ref: undefined,
		children: [[{ label: "Menu not yet populated" }]],
	},
	{
		label: "Help",
		ref: undefined,
		children: [[{ label: "Menu not yet populated" }]],
	},
];

export default defineComponent({
	methods: {
		setEntryRefs(menuEntry: MenuListEntry, ref: typeof MenuList) {
			if (ref) menuEntry.ref = ref;
		},
		handleEntryClick(menuEntry: MenuListEntry) {
			if (menuEntry.ref) menuEntry.ref.setOpen();
			else throw new Error("The menu bar floating menu has no associated ref");
		},
		handleLogoClick() {
			window.open("https://www.graphite.design", "_blank");
		},
		actionNotImplemented() {
			// eslint-disable-next-line no-alert
			alert("This action is not yet implemented");
		},
	},
	data() {
		return {
			ApplicationPlatform,
			menuEntries,
			MenuDirection,
		};
	},
	components: {
		IconLabel,
		MenuList,
	},
});
</script>

<template>
	<div class="menu-bar-input">
		<div class="entry-container">
			<div @click="visitWebsite('https://www.graphite.design')" class="entry">
				<IconLabel :icon="'GraphiteLogo'" />
			</div>
		</div>
		<div class="entry-container" v-for="entry in menuEntries" :key="entry">
			<div @click="handleEntryClick(entry)" class="entry" :class="{ open: entry.ref && entry.ref.isOpen() }" data-hover-menu-spawner>
				<IconLabel :icon="entry.icon" v-if="entry.icon" />
				<span v-if="entry.label">{{ entry.label }}</span>
			</div>
			<MenuList :menuEntries="entry.children" :direction="MenuDirection.Bottom" :minWidth="240" :drawIcon="true" :defaultAction="dialog.comingSoon" :ref="(ref) => setEntryRefs(entry, ref)" />
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

import { EditorWasm } from "@/utilities/wasm-loader";

import IconLabel from "@/components/widgets/labels/IconLabel.vue";
import { ApplicationPlatform } from "@/components/window/MainWindow.vue";
import MenuList, { MenuListEntry, MenuListEntries } from "@/components/widgets/floating-menus/MenuList.vue";
import { MenuDirection } from "@/components/widgets/floating-menus/FloatingMenu.vue";

function makeMenuEntries(editor: EditorWasm): MenuListEntries {
	return [
		{
			label: "File",
			ref: undefined,
			children: [
				[
					{ label: "New", icon: "File", shortcut: ["Ctrl", "N"], shortcutRequiresLock: true, action: async () => editor.new_document() },
					{ label: "Open…", shortcut: ["Ctrl", "O"], action: async () => editor.open_document() },
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
					{ label: "Close", shortcut: ["Ctrl", "W"], shortcutRequiresLock: true, action: async () => editor.close_active_document_with_confirmation() },
					{ label: "Close All", shortcut: ["Ctrl", "Alt", "W"], action: async () => editor.close_all_documents_with_confirmation() },
				],
				[
					{ label: "Save", shortcut: ["Ctrl", "S"], action: async () => editor.save_document() },
					{ label: "Save As…", shortcut: ["Ctrl", "⇧", "S"], action: async () => editor.save_document() },
					{ label: "Save All", shortcut: ["Ctrl", "Alt", "S"] },
					{ label: "Auto-Save", checkbox: true, checked: true },
				],
				[
					{ label: "Import…", shortcut: ["Ctrl", "I"] },
					{ label: "Export…", shortcut: ["Ctrl", "E"], action: async () => editor.export_document() },
				],
				[{ label: "Quit", shortcut: ["Ctrl", "Q"] }],
			],
		},
		{
			label: "Edit",
			ref: undefined,
			children: [
				[
					{ label: "Undo", shortcut: ["Ctrl", "Z"], action: async () => editor.undo() },
					{ label: "Redo", shortcut: ["Ctrl", "⇧", "Z"], action: async () => editor.redo() },
				],
				[
					{ label: "Cut", shortcut: ["Ctrl", "X"] },
					{ label: "Copy", icon: "Copy", shortcut: ["Ctrl", "C"] },
					{ label: "Paste", icon: "Paste", shortcut: ["Ctrl", "V"] },
				],
			],
		},
		{
			label: "Layer",
			ref: undefined,
			children: [
				[
					{ label: "Select All", shortcut: ["Ctrl", "A"], action: async () => editor.select_all_layers() },
					{ label: "Deselect All", shortcut: ["Ctrl", "Alt", "A"], action: async () => editor.deselect_all_layers() },
					{
						label: "Order",
						children: [
							[
								{ label: "Raise To Front", shortcut: ["Ctrl", "Shift", "]"], action: async () => editor.reorder_selected_layers(editor.i32_max()) },
								{ label: "Raise", shortcut: ["Ctrl", "]"], action: async () => editor.reorder_selected_layers(1) },
								{ label: "Lower", shortcut: ["Ctrl", "["], action: async () => editor.reorder_selected_layers(-1) },
								{ label: "Lower to Back", shortcut: ["Ctrl", "Shift", "["], action: async () => editor.reorder_selected_layers(editor.i32_min()) },
							],
						],
					},
				],
			],
		},
		{
			label: "Document",
			ref: undefined,
			children: [[{ label: "Menu entries coming soon" }]],
		},
		{
			label: "View",
			ref: undefined,
			children: [[{ label: "Menu entries coming soon" }]],
		},
		{
			label: "Help",
			ref: undefined,
			children: [
				[
					{ label: "Report a Bug", action: () => window.open("https://github.com/GraphiteEditor/Graphite/issues/new", "_blank") },
					{ label: "Visit on GitHub", action: () => window.open("https://github.com/GraphiteEditor/Graphite", "_blank") },
				],
				[
					{ label: "Graphite License", action: () => window.open("https://raw.githubusercontent.com/GraphiteEditor/Graphite/master/LICENSE.txt", "_blank") },
					{ label: "Third-Party Licenses", action: () => window.open("/third-party-licenses.txt", "_blank") },
				],
				[{ label: "Debug: Panic (DANGER)", action: async () => editor.intentional_panic() }],
			],
		},
	];
}

export default defineComponent({
	inject: ["editor", "dialog"],
	methods: {
		setEntryRefs(menuEntry: MenuListEntry, ref: typeof MenuList) {
			if (ref) menuEntry.ref = ref;
		},
		handleEntryClick(menuEntry: MenuListEntry) {
			if (menuEntry.ref) menuEntry.ref.setOpen();
			else throw new Error("The menu bar floating menu has no associated ref");
		},
		visitWebsite(url: string) {
			// This method is required because `window` isn't accessible from the Vue component HTML
			window.open(url, "_blank");
		},
	},
	data() {
		return {
			ApplicationPlatform,
			menuEntries: makeMenuEntries(this.editor),
			MenuDirection,
			comingSoon: this.dialog.comingSoon,
		};
	},
	components: {
		IconLabel,
		MenuList,
	},
});
</script>

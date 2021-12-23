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
			<MenuList :menuEntries="entry.children" :direction="MenuDirection.Bottom" :minWidth="240" :drawIcon="true" :defaultAction="comingSoon" :ref="(ref) => setEntryRefs(entry, ref)" />
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
import { EditorState } from "@/state/wasm-loader";

function makeMenuEntries(editor: EditorState): MenuListEntries {
	return [
		{
			label: "File",
			ref: undefined,
			children: [
				[
					{ label: "New", icon: "File", shortcut: ["KeyControl", "KeyN"], shortcutRequiresLock: true, action: async () => editor.instance.new_document() },
					{ label: "Open…", shortcut: ["KeyControl", "KeyO"], action: async () => editor.instance.open_document() },
					{
						label: "Open Recent",
						shortcut: ["KeyControl", "KeyShift", "KeyO"],
						children: [
							[{ label: "Reopen Last Closed", shortcut: ["KeyControl", "KeyShift", "KeyT"], shortcutRequiresLock: true }, { label: "Clear Recently Opened" }],
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
					{ label: "Close", shortcut: ["KeyControl", "KeyW"], shortcutRequiresLock: true, action: async () => editor.instance.close_active_document_with_confirmation() },
					{ label: "Close All", shortcut: ["KeyControl", "KeyAlt", "KeyW"], action: async () => editor.instance.close_all_documents_with_confirmation() },
				],
				[
					{ label: "Save", shortcut: ["KeyControl", "KeyS"], action: async () => editor.instance.save_document() },
					{ label: "Save As…", shortcut: ["KeyControl", "KeyShift", "KeyS"], action: async () => editor.instance.save_document() },
					{ label: "Save All", shortcut: ["KeyControl", "KeyAlt", "KeyS"] },
					{ label: "Auto-Save", checkbox: true, checked: true },
				],
				[
					{ label: "Import…", shortcut: ["KeyControl", "KeyI"] },
					{ label: "Export…", shortcut: ["KeyControl", "KeyE"], action: async () => editor.instance.export_document() },
				],
				[{ label: "Quit", shortcut: ["KeyControl", "KeyQ"] }],
			],
		},
		{
			label: "Edit",
			ref: undefined,
			children: [
				[
					{ label: "Undo", shortcut: ["KeyControl", "KeyZ"], action: async () => editor.instance.undo() },
					{ label: "Redo", shortcut: ["KeyControl", "KeyShift", "KeyZ"], action: async () => editor.instance.redo() },
				],
				[
					{ label: "Cut", shortcut: ["KeyControl", "KeyX"] },
					{ label: "Copy", icon: "Copy", shortcut: ["KeyControl", "KeyC"] },
					{ label: "Paste", icon: "Paste", shortcut: ["KeyControl", "KeyV"] },
				],
			],
		},
		{
			label: "Layer",
			ref: undefined,
			children: [
				[
					{ label: "Select All", shortcut: ["KeyControl", "KeyA"], action: async () => editor.instance.select_all_layers() },
					{ label: "Deselect All", shortcut: ["KeyControl", "KeyAlt", "KeyA"], action: async () => editor.instance.deselect_all_layers() },
					{
						label: "Order",
						children: [
							[
								{
									label: "Raise To Front",
									shortcut: ["KeyControl", "KeyShift", "KeyLeftBracket"],
									action: async () => editor.instance.reorder_selected_layers(editor.rawWasm.i32_max()),
								},
								{ label: "Raise", shortcut: ["KeyControl", "KeyRightBracket"], action: async () => editor.instance.reorder_selected_layers(1) },
								{ label: "Lower", shortcut: ["KeyControl", "KeyLeftBracket"], action: async () => editor.instance.reorder_selected_layers(-1) },
								{
									label: "Lower to Back",
									shortcut: ["KeyControl", "KeyShift", "KeyRightBracket"],
									action: async () => editor.instance.reorder_selected_layers(editor.rawWasm.i32_min()),
								},
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
				[{ label: "About Graphite", action: async () => editor.instance.request_about_graphite_dialog() }],
				[
					{ label: "Report a Bug", action: () => window.open("https://github.com/GraphiteEditor/Graphite/issues/new", "_blank") },
					{ label: "Visit on GitHub", action: () => window.open("https://github.com/GraphiteEditor/Graphite", "_blank") },
				],
				[{ label: "Debug: Panic (DANGER)", action: async () => editor.rawWasm.intentional_panic() }],
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
			comingSoon: () => this.dialog.comingSoon(),
		};
	},
	components: {
		IconLabel,
		MenuList,
	},
});
</script>

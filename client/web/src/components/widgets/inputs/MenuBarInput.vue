<template>
	<div class="menu-bar-input">
		<div class="entry-container" v-for="entry in menuEntries" :key="entry">
			<div @click="handleEntryClick(entry)" class="entry" :class="{ open: entry.ref && entry.ref.isOpen() }" data-hover-menu-spawner>
				<Icon :icon="entry.icon" v-if="entry.icon" />
				<span v-if="entry.label">{{ entry.label }}</span>
			</div>
			<MenuList
				:ourEntry="entry"
				:menuEntries="entry.children"
				:direction="MenuDirection.Bottom"
				:minWidth="240"
				:drawIcon="true"
				:defaultAction="actionNotImplemented"
				:ref="(ref) => setEntryRefs(entry, ref)"
			/>
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
import Icon from "../labels/Icon.vue";
import { ApplicationPlatform } from "../../window/MainWindow.vue";
import MenuList, { MenuListEntry, MenuListEntries } from "../floating-menus/MenuList.vue";
import { MenuDirection } from "../floating-menus/FloatingMenu.vue";

const wasm = import("../../../../wasm/pkg");

const menuEntries: MenuListEntries = [
	{
		id: "graphite",
		icon: "GraphiteLogo",
		ref: undefined,
		children: [[{ id: "graphite/github", label: "Visit project GitHub…", action: () => window.open("https://github.com/GraphiteEditor/Graphite", "_blank") }]],
	},
	{
		id: "file",
		label: "File",
		ref: undefined,
		children: [
			[
				{ id: "file/new", label: "New", icon: "File", shortcut: ["Ctrl", "N"], action: async () => (await wasm).new_document() },
				{ id: "file/open", label: "Open…", shortcut: ["Ctrl", "O"] },
				{
					id: "file/open_recent",
					label: "Open Recent",
					shortcut: ["Ctrl", "⇧", "O"],
					children: [
						[
							{ id: "file/open_recent/reopen", label: "Reopen Last Closed", shortcut: ["Ctrl", "⇧", "T"] },
							{ id: "file/open_recent/clear_recent", label: "Clear Recently Opened" },
						],
						[
							{ id: "file/open_recent/1", label: "Some Recent File.gdd" },
							{ id: "file/open_recent/2", label: "Another Recent File.gdd" },
							{ id: "file/open_recent/3", label: "An Older File.gdd" },
							{ id: "file/open_recent/4", label: "Some Other Older File.gdd" },
							{ id: "file/open_recent/5", label: "Yet Another Older File.gdd" },
						],
					],
				},
			],
			[
				{ id: "file/close", label: "Close", shortcut: ["Ctrl", "W"] },
				{ id: "file/close_all", label: "Close All", shortcut: ["Ctrl", "Alt", "W"] },
			],
			[
				{ id: "file/save", label: "Save", shortcut: ["Ctrl", "S"] },
				{ id: "file/save_as", label: "Save As…", shortcut: ["Ctrl", "⇧", "S"] },
				{ id: "file/save_all", label: "Save All", shortcut: ["Ctrl", "Alt", "S"] },
				{ id: "file/auto_save", label: "Auto-Save", shortcut: undefined },
			],
			[
				{ id: "file/import", label: "Import…", shortcut: ["Ctrl", "I"] },
				{ id: "file/export", label: "Export…", shortcut: ["Ctrl", "E"], action: async () => (await wasm).export_document() },
			],
			[{ id: "file/quit", label: "Quit", shortcut: ["Ctrl", "Q"] }],
		],
	},
	{
		id: "edit",
		label: "Edit",
		ref: undefined,
		children: [
			[
				{ id: "edit/undo", label: "Undo", shortcut: ["Ctrl", "Z"], action: async () => (await wasm).undo() },
				{ id: "edit/redo", label: "Redo", shortcut: ["Ctrl", "⇧", "Z"] },
			],
			[
				{ id: "edit/cut", label: "Cut", shortcut: ["Ctrl", "X"] },
				{ id: "edit/copy", label: "Copy", icon: "Copy", shortcut: ["Ctrl", "C"] },
				{ id: "edit/paste", label: "Paste", icon: "Paste", shortcut: ["Ctrl", "V"] },
			],
		],
	},
	{
		id: "document",
		label: "Document",
		ref: undefined,
		children: [[{ id: "document/todo", label: "Menu not yet populated" }]],
	},
	{
		id: "view",
		label: "View",
		ref: undefined,
		children: [[{ id: "document/todo", label: "Menu not yet populated" }]],
	},
	{
		id: "help",
		label: "Help",
		ref: undefined,
		children: [[{ id: "document/todo", label: "Menu not yet populated" }]],
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
		actionNotImplemented() {
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
		Icon,
		MenuList,
	},
});
</script>

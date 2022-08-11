<template>
	<div class="menu-bar-input" data-menu-bar-input>
		<div class="entry-container">
			<button @click="() => visitWebsite('https://graphite.rs')" class="entry">
				<IconLabel :icon="'GraphiteLogo'" />
			</button>
		</div>
		<div class="entry-container" v-for="(entry, index) in entries" :key="index">
			<div
				@click="(e) => onClick(entry, e.target)"
				tabindex="0"
				@blur="(e: FocusEvent) => blur(e,entry)"
				@keydown="entry.ref?.keydown"
				class="entry"
				:class="{ open: entry.ref?.isOpen }"
				data-hover-menu-spawner
			>
				<IconLabel v-if="entry.icon" :icon="entry.icon" />
				<span v-if="entry.label">{{ entry.label }}</span>
			</div>
			<MenuList
				:open="entry.ref?.open || false"
				:entries="entry.children || []"
				:direction="'Bottom'"
				:minWidth="240"
				:drawIcon="true"
				:defaultAction="() => editor.instance.request_coming_soon_dialog()"
				:ref="(ref: typeof MenuList) => ref && (entry.ref = ref)"
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
			background: none;
			border: 0;
			margin: 0;

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

import { platformIsMac } from "@/utility-functions/platform";
import { MenuEntry, UpdateMenuBarLayout, MenuListEntry, KeyRaw, KeysGroup } from "@/wasm-communication/messages";

import MenuList from "@/components/floating-menus/MenuList.vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";

// TODO: Apparently, Safari does not support the Keyboard.lock() API but does relax its authority over certain keyboard shortcuts in fullscreen mode, which we should take advantage of
const accelKey = platformIsMac() ? "Command" : "Control";
const LOCK_REQUIRING_SHORTCUTS: KeyRaw[][] = [
	[accelKey, "KeyW"],
	[accelKey, "KeyN"],
	[accelKey, "Shift", "KeyN"],
	[accelKey, "KeyT"],
	[accelKey, "Shift", "KeyT"],
];

type FrontendMenuColumn = {
	label: string;
	children: FrontendMenuEntry[][];
};
type FrontendMenuEntry = Omit<MenuEntry, "action" | "children"> & { shortcutRequiresLock: boolean | undefined; action: () => void; children: FrontendMenuEntry[][] | undefined };

export default defineComponent({
	inject: ["editor"],
	mounted() {
		this.editor.subscriptions.subscribeJsMessage(UpdateMenuBarLayout, (updateMenuBarLayout) => {
			const arraysEqual = (a: KeyRaw[], b: KeyRaw[]): boolean => a.length === b.length && a.every((aValue, i) => aValue === b[i]);
			const shortcutRequiresLock = (shortcut: KeysGroup): boolean => {
				const shortcutKeys = shortcut.map((keyWithLabel) => keyWithLabel.key);

				// If this shortcut matches any of the browser-reserved shortcuts
				return LOCK_REQUIRING_SHORTCUTS.some((lockKeyCombo) => arraysEqual(shortcutKeys, lockKeyCombo));
			};

			const menuEntryToFrontendMenuEntry = (subLayout: MenuEntry[][]): FrontendMenuEntry[][] =>
				subLayout.map((group) =>
					group.map((entry) => ({
						...entry,
						children: entry.children ? menuEntryToFrontendMenuEntry(entry.children) : undefined,
						action: (): void => this.editor.instance.update_layout(updateMenuBarLayout.layout_target, entry.action.widgetId, undefined),
						shortcutRequiresLock: entry.shortcut ? shortcutRequiresLock(entry.shortcut.keys) : undefined,
					}))
				);

			this.entries = updateMenuBarLayout.layout.map((column) => ({ ...column, children: menuEntryToFrontendMenuEntry(column.children) }));
		});
	},
	methods: {
		onClick(menuEntry: MenuListEntry, target: EventTarget | null) {
			// Focus the target so that keyboard inputs are sent to the dropdown
			(target as HTMLElement)?.focus();

			if (menuEntry.ref) menuEntry.ref.isOpen = true;
			else throw new Error("The menu bar floating menu has no associated ref");
		},
		blur(e: FocusEvent, menuEntry: MenuListEntry) {
			if ((e.target as HTMLElement).closest("[data-menu-bar-input]") !== this.$el && menuEntry.ref) menuEntry.ref.isOpen = false;
		},
		// TODO: Move to backend
		visitWebsite(url: string) {
			// This method is required because `window` isn't accessible from the Vue component HTML
			window.open(url, "_blank");
		},
	},
	data() {
		return {
			entries: [] as FrontendMenuColumn[],
			open: false,
		};
	},
	components: {
		IconLabel,
		MenuList,
	},
});
</script>

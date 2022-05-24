<template>
	<FloatingMenu
		class="menu-list"
		v-model:open="isOpen"
		@naturalWidth="(newNaturalWidth: number) => $emit('naturalWidth', newNaturalWidth)"
		:type="'Dropdown'"
		:windowEdgeMargin="0"
		v-bind="{ direction, scrollableY, minWidth }"
		ref="floatingMenu"
		data-hover-menu-keep-open
	>
		<template v-for="(section, sectionIndex) in entries" :key="sectionIndex">
			<Separator :type="'List'" :direction="'Vertical'" v-if="sectionIndex > 0" />
			<LayoutRow
				v-for="(entry, entryIndex) in section"
				:key="entryIndex"
				class="row"
				:class="{ open: isEntryOpen(entry), active: entry.label === activeEntry?.label }"
				@click="() => onEntryClick(entry)"
				@pointerenter="() => onEntryPointerEnter(entry)"
				@pointerleave="() => onEntryPointerLeave(entry)"
			>
				<CheckboxInput v-if="entry.checkbox" v-model:checked="entry.checked" :outlineStyle="true" class="entry-checkbox" />
				<IconLabel v-else-if="entry.icon && drawIcon" :icon="entry.icon" class="entry-icon" />
				<div v-else-if="drawIcon" class="no-icon"></div>

				<span class="entry-label">{{ entry.label }}</span>

				<IconLabel v-if="entry.shortcutRequiresLock && !fullscreen.state.keyboardLocked" :icon="'Info'" :title="keyboardLockInfoMessage" />
				<UserInputLabel v-else-if="entry.shortcut?.length" :inputKeys="[entry.shortcut]" />

				<div class="submenu-arrow" v-if="entry.children?.length"></div>
				<div class="no-submenu-arrow" v-else></div>

				<MenuList
					v-if="entry.children"
					@naturalWidth="(newNaturalWidth: number) => $emit('naturalWidth', newNaturalWidth)"
					:open="entry.ref?.open || false"
					:direction="'TopRight'"
					:entries="entry.children"
					v-bind="{ defaultAction, minWidth, drawIcon, scrollableY }"
					:ref="(ref: typeof FloatingMenu) => ref && (entry.ref = ref)"
				/>
			</LayoutRow>
		</template>
	</FloatingMenu>
</template>

<style lang="scss">
.menu-list {
	.floating-menu-container .floating-menu-content {
		padding: 4px 0;

		.row {
			height: 20px;
			align-items: center;
			white-space: nowrap;
			position: relative;
			flex: 0 0 auto;

			& > * {
				flex: 0 0 auto;
			}

			.entry-icon svg {
				fill: var(--color-e-nearwhite);
			}

			.no-icon {
				width: 16px;
			}

			.entry-label {
				flex: 1 1 100%;
				margin-left: 8px;
			}

			.entry-checkbox,
			.entry-icon,
			.no-icon {
				margin: 0 4px;

				& + .entry-label {
					margin-left: 0;
				}
			}

			.user-input-label {
				margin: 0;
				margin-left: 16px;
			}

			.submenu-arrow {
				width: 0;
				height: 0;
				border-style: solid;
				border-width: 3px 0 3px 6px;
				border-color: transparent transparent transparent var(--color-e-nearwhite);
			}

			.no-submenu-arrow {
				width: 6px;
			}

			.submenu-arrow,
			.no-submenu-arrow {
				margin-left: 6px;
				margin-right: 4px;
			}

			&:hover,
			&.open,
			&.active {
				background: var(--color-6-lowergray);

				&.active {
					background: var(--color-accent);
				}

				svg {
					fill: var(--color-f-white);
				}

				span {
					color: var(--color-f-white);
				}
			}

			&:hover .entry-checkbox label .checkbox-box {
				border: 1px solid var(--color-f-white);

				svg {
					fill: var(--color-f-white);
				}
			}
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { IconName } from "@/utility-functions/icons";

import FloatingMenu, { MenuDirection } from "@/components/floating-menus/FloatingMenu.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";
import CheckboxInput from "@/components/widgets/inputs/CheckboxInput.vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";
import Separator from "@/components/widgets/labels/Separator.vue";
import UserInputLabel from "@/components/widgets/labels/UserInputLabel.vue";

export type MenuListEntries<Value = string> = MenuListEntry<Value>[];
export type SectionsOfMenuListEntries<Value = string> = MenuListEntries<Value>[];

interface MenuListEntryData<Value = string> {
	value?: Value;
	label?: string;
	icon?: IconName;
	checkbox?: boolean;
	shortcut?: string[];
	shortcutRequiresLock?: boolean;
	action?: () => void;
	children?: SectionsOfMenuListEntries;
}

export type MenuListEntry<Value = string> = MenuListEntryData<Value> & { ref?: typeof FloatingMenu | typeof MenuList; checked?: boolean };

const KEYBOARD_LOCK_USE_FULLSCREEN = "This hotkey is reserved by the browser, but becomes available in fullscreen mode";
const KEYBOARD_LOCK_SWITCH_BROWSER = "This hotkey is reserved by the browser, but becomes available in Chrome, Edge, and Opera which support the Keyboard.lock() API";

const MenuList = defineComponent({
	inject: ["fullscreen"],
	emits: ["update:open", "update:activeEntry", "naturalWidth"],
	props: {
		entries: { type: Array as PropType<SectionsOfMenuListEntries>, required: true },
		activeEntry: { type: Object as PropType<MenuListEntry>, required: false },
		open: { type: Boolean as PropType<boolean>, required: true },
		direction: { type: String as PropType<MenuDirection>, default: "Bottom" },
		minWidth: { type: Number as PropType<number>, default: 0 },
		drawIcon: { type: Boolean as PropType<boolean>, default: false },
		scrollableY: { type: Boolean as PropType<boolean>, default: false },
		defaultAction: { type: Function as PropType<() => void>, required: false },
	},
	data() {
		return {
			isOpen: this.open,
			keyboardLockInfoMessage: this.fullscreen.keyboardLockApiSupported ? KEYBOARD_LOCK_USE_FULLSCREEN : KEYBOARD_LOCK_SWITCH_BROWSER,
		};
	},
	watch: {
		// Called only when `open` is changed from outside this component (with v-model)
		open(newOpen: boolean) {
			this.isOpen = newOpen;
		},
		isOpen(newIsOpen: boolean) {
			this.$emit("update:open", newIsOpen);
		},
		entries() {
			const floatingMenu = this.$refs.floatingMenu as typeof FloatingMenu;
			floatingMenu.measureAndEmitNaturalWidth();
		},
		drawIcon() {
			const floatingMenu = this.$refs.floatingMenu as typeof FloatingMenu;
			floatingMenu.measureAndEmitNaturalWidth();
		},
	},
	methods: {
		onEntryClick(menuEntry: MenuListEntry): void {
			// Toggle checkbox
			// TODO: This is broken at the moment, fix it when we get rid of using `ref`
			if (menuEntry.checkbox) menuEntry.checked = !menuEntry.checked;

			// Call the action, or a default, if either are provided
			if (menuEntry.action) menuEntry.action();
			else if (this.defaultAction) this.defaultAction();

			// Emit the clicked entry as the new active entry
			this.$emit("update:activeEntry", menuEntry);

			// Close the containing menu
			if (menuEntry.ref) menuEntry.ref.isOpen = false;
			this.$emit("update:open", false);
			this.isOpen = false; // TODO: This is a hack for MenuBarInput submenus, remove it when we get rid of using `ref`
		},
		onEntryPointerEnter(menuEntry: MenuListEntry): void {
			if (!menuEntry.children?.length) return;

			if (menuEntry.ref) menuEntry.ref.isOpen = true;
			else this.$emit("update:open", true);
		},
		onEntryPointerLeave(menuEntry: MenuListEntry): void {
			if (!menuEntry.children?.length) return;

			if (menuEntry.ref) menuEntry.ref.isOpen = false;
			else this.$emit("update:open", false);
		},
		isEntryOpen(menuEntry: MenuListEntry): boolean {
			if (!menuEntry.children?.length) return false;

			return this.open;
		},
	},
	computed: {
		entriesWithoutRefs(): MenuListEntryData[][] {
			return this.entries.map((menuListEntries) =>
				menuListEntries.map((entry) => {
					const { ref, ...entryWithoutRef } = entry;
					return entryWithoutRef;
				})
			);
		},
	},
	components: {
		FloatingMenu,
		Separator,
		IconLabel,
		CheckboxInput,
		UserInputLabel,
		LayoutRow,
	},
});
export default MenuList;
</script>

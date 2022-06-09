<template>
	<FloatingMenu
		class="menu-list"
		v-model:open="isOpen"
		@naturalWidth="(newNaturalWidth: number) => $emit('naturalWidth', newNaturalWidth)"
		:type="'Dropdown'"
		:windowEdgeMargin="0"
		:escapeCloses="false"
		v-bind="{ direction, minWidth, scrollableY: !fixedHeight && scrollableY }"
		ref="floatingMenu"
		data-hover-menu-keep-open
	>
		<LayoutCol ref="scroller" :scrollableY="scrollableY && fixedHeight" @scroll="onScroll" :style="{ minWidth: fixedHeight ? `${minWidth}px` : `inherit` }">
			<LayoutRow v-if="fixedHeight" class="spacer" :style="{ height: `${startIndex * fixedHeight}px` }"></LayoutRow>
			<template v-for="(section, sectionIndex) in entries" :key="sectionIndex">
				<Separator :type="'List'" :direction="'Vertical'" v-if="sectionIndex > 0" />
				<LayoutRow
					v-for="(entry, entryIndex) in fixedHeight ? section.slice(startIndex, endIndex) : section"
					:key="entryIndex + (fixedHeight ? startIndex : 0)"
					class="row"
					:class="{ open: isEntryOpen(entry), active: entry.label === highlighted?.label }"
					:style="{ height: fixedHeight || '20px' }"
					@click="() => onEntryClick(entry)"
					@pointerenter="() => onEntryPointerEnter(entry)"
					@pointerleave="() => onEntryPointerLeave(entry)"
				>
					<CheckboxInput v-if="entry.checkbox" v-model:checked="entry.checked" :outlineStyle="true" :disableTabIndex="true" class="entry-checkbox" />
					<IconLabel v-else-if="entry.icon && drawIcon" :icon="entry.icon" class="entry-icon" />
					<div v-else-if="drawIcon" class="no-icon"></div>

					<link v-if="entry.font" rel="stylesheet" :href="entry.font?.toString()" />

					<span class="entry-label" :style="{ fontFamily: `${!entry.font ? 'inherit' : entry.value}` }">{{ entry.label }}</span>

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
			<LayoutRow v-if="fixedHeight" class="spacer" :style="{ height: `${totalHeight - endIndex * fixedHeight}px` }"></LayoutRow>
		</LayoutCol>
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
import LayoutCol from "@/components/layout/LayoutCol.vue";
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
	font?: URL;
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
		interactive: { type: Boolean as PropType<boolean>, default: false },
		scrollableY: { type: Boolean as PropType<boolean>, default: false },
		fixedHeight: { type: Number as PropType<number>, default: 0 },
		defaultAction: { type: Function as PropType<() => void>, required: false },
	},
	data() {
		return {
			isOpen: this.open,
			keyboardLockInfoMessage: this.fullscreen.keyboardLockApiSupported ? KEYBOARD_LOCK_USE_FULLSCREEN : KEYBOARD_LOCK_SWITCH_BROWSER,
			highlighted: this.activeEntry as MenuListEntry | undefined,
			entriesStart: 0,
		};
	},
	watch: {
		// Called only when `open` is changed from outside this component (with v-model)
		open(newOpen: boolean) {
			this.isOpen = newOpen;
			this.highlighted = this.activeEntry;
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

		/// Handles keyboard navigation for the menu. Returns if the entire menu stack should be dismissed
		keydown(e: KeyboardEvent, submenu: boolean): boolean {
			// Interactive menus should keep the active entry the same as the highlighted one
			if (this.interactive) this.highlighted = this.activeEntry;

			const menuOpen = this.isOpen;
			const flatEntries = this.entries.flat();
			const openChild = flatEntries.findIndex((entry) => entry.children?.length && entry.ref?.isOpen);

			const openSubmenu = (highlighted: MenuListEntry<string>): void => {
				if (highlighted.ref && highlighted.children?.length) {
					highlighted.ref.isOpen = true;

					// Highlight first item
					highlighted.ref.setHighlighted(highlighted.children[0][0]);
				}
			};

			if (!menuOpen && (e.key === " " || e.key === "Enter")) {
				// Allow opening menu with space or enter
				this.isOpen = true;
				this.highlighted = this.activeEntry;
			} else if (menuOpen && openChild >= 0) {
				// Redirect the keyboard navigation to a submenu if one is open
				const shouldCloseStack = flatEntries[openChild].ref?.keydown(e, true);

				// Highlight the menu item in the parent list that corresponds with the open submenu
				if (e.key !== "Escape" && this.highlighted) this.setHighlighted(flatEntries[openChild]);

				// Handle the child closing the entire menu stack
				if (shouldCloseStack) {
					this.isOpen = false;
					return true;
				}
			} else if ((menuOpen || this.interactive) && (e.key === "ArrowUp" || e.key === "ArrowDown")) {
				// Navigate to the next and previous entries with arrow keys

				let newIndex = e.key === "ArrowUp" ? flatEntries.length - 1 : 0;
				if (this.highlighted) {
					const index = this.highlighted ? flatEntries.map((entry) => entry.label).indexOf(this.highlighted.label) : 0;
					newIndex = index + (e.key === "ArrowUp" ? -1 : 1);

					// Interactive dropdowns should lock at the end whereas other dropdowns should loop
					if (this.interactive) newIndex = Math.min(flatEntries.length - 1, Math.max(0, newIndex));
					else newIndex = (newIndex + flatEntries.length) % flatEntries.length;
				}

				const newEntry = flatEntries[newIndex];
				this.setHighlighted(newEntry);
			} else if (menuOpen && e.key === "Escape") {
				// Close menu with escape key
				this.isOpen = false;

				// Reset active to before open
				this.setHighlighted(this.activeEntry);
			} else if (menuOpen && this.highlighted && e.key === "Enter") {
				// Handle clicking on an option if enter is pressed
				if (!this.highlighted.children?.length) this.onEntryClick(this.highlighted);
				else openSubmenu(this.highlighted);

				// Stop the event from triggering a press on a new dialog
				e.preventDefault();

				// Enter should close the entire menu stack
				return true;
			} else if (menuOpen && this.highlighted && e.key === "ArrowRight") {
				// Right arrow opens a submenu
				openSubmenu(this.highlighted);
			} else if (menuOpen && e.key === "ArrowLeft") {
				// Left arrow closes a submenu
				if (submenu) this.isOpen = false;
			}

			// By default, keep the menu stack open
			return false;
		},
		setHighlighted(newHighlight: MenuListEntry<string> | undefined) {
			this.highlighted = newHighlight;
			// Interactive menus should keep the active entry the same as the highlighted one
			if (this.interactive && newHighlight?.value !== this.activeEntry?.value) this.$emit("update:activeEntry", newHighlight);
		},
		onScroll(e: Event) {
			if (!this.fixedHeight) return;
			this.entriesStart = (e.target as HTMLElement)?.scrollTop || 0;
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
		totalHeight() {
			return this.entries[0].length * this.fixedHeight;
		},
		startIndex() {
			return Math.floor(this.entriesStart / this.fixedHeight);
		},
		endIndex() {
			return Math.min(this.entries[0].length, this.startIndex + 1 + 400 / this.fixedHeight);
		},
	},
	components: {
		FloatingMenu,
		Separator,
		IconLabel,
		CheckboxInput,
		UserInputLabel,
		LayoutRow,
		LayoutCol,
	},
});
export default MenuList;
</script>

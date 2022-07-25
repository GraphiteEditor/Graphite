<template>
	<FloatingMenu
		class="menu-list"
		v-model:open="isOpen"
		@naturalWidth="(newNaturalWidth: number) => $emit('naturalWidth', newNaturalWidth)"
		:type="'Dropdown'"
		:windowEdgeMargin="0"
		:escapeCloses="false"
		v-bind="{ direction, scrollableY: scrollableY && virtualScrollingEntryHeight === 0, minWidth }"
		ref="floatingMenu"
		data-hover-menu-keep-open
	>
		<!-- If we put the scrollableY on the layoutcol for non-font dropdowns then for some reason it always creates a tiny scrollbar.
		However when we are using the virtual scrolling then we need the layoutcol to be scrolling so we can bind the events without using $refs. -->
		<LayoutCol ref="scroller" :scrollableY="scrollableY && virtualScrollingEntryHeight !== 0" @scroll="onScroll" :style="{ minWidth: virtualScrollingEntryHeight ? `${minWidth}px` : `inherit` }">
			<LayoutRow v-if="virtualScrollingEntryHeight" class="scroll-spacer" :style="{ height: `${virtualScrollingStartIndex * virtualScrollingEntryHeight}px` }"></LayoutRow>
			<template v-for="(section, sectionIndex) in entries" :key="sectionIndex">
				<Separator :type="'List'" :direction="'Vertical'" v-if="sectionIndex > 0" />
				<LayoutRow
					v-for="(entry, entryIndex) in virtualScrollingEntryHeight ? section.slice(virtualScrollingStartIndex, virtualScrollingEndIndex) : section"
					:key="entryIndex + (virtualScrollingEntryHeight ? virtualScrollingStartIndex : 0)"
					class="row"
					:class="{ open: isEntryOpen(entry), active: entry.label === highlighted?.label, disabled: entry.disabled }"
					:style="{ height: virtualScrollingEntryHeight || '20px' }"
					@click="() => !entry.disabled && onEntryClick(entry)"
					@pointerenter="() => !entry.disabled && onEntryPointerEnter(entry)"
					@pointerleave="() => !entry.disabled && onEntryPointerLeave(entry)"
				>
					<IconLabel v-if="entry.icon && drawIcon" :icon="entry.icon" class="entry-icon" />
					<div v-else-if="drawIcon" class="no-icon"></div>

					<link v-if="entry.font" rel="stylesheet" :href="entry.font?.toString()" />

					<span class="entry-label" :style="{ fontFamily: `${!entry.font ? 'inherit' : entry.value}` }">{{ entry.label }}</span>

					<UserInputLabel v-if="entry.shortcut?.length" :inputKeys="[entry.shortcut]" :requiresLock="entry.shortcutRequiresLock" />

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
			<LayoutRow
				v-if="virtualScrollingEntryHeight"
				class="scroll-spacer"
				:style="{ height: `${virtualScrollingTotalHeight - virtualScrollingEndIndex * virtualScrollingEntryHeight}px` }"
			></LayoutRow>
		</LayoutCol>
	</FloatingMenu>
</template>

<style lang="scss">
.menu-list {
	.floating-menu-container .floating-menu-content {
		padding: 4px 0;

		.scroll-spacer {
			flex: 0 0 auto;
		}

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
				color: var(--color-f-white);

				&.active {
					background: var(--color-accent);
				}

				svg {
					fill: var(--color-f-white);
				}
			}

			&.disabled {
				color: var(--color-8-uppergray);

				&:hover {
					background: none;
				}

				svg {
					fill: var(--color-8-uppergray);
				}
			}
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { MenuListEntry, SectionsOfMenuListEntries, MenuListEntryData } from "@/wasm-communication/messages";

import FloatingMenu, { MenuDirection } from "@/components/floating-menus/FloatingMenu.vue";
import LayoutCol from "@/components/layout/LayoutCol.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";
import Separator from "@/components/widgets/labels/Separator.vue";
import UserInputLabel from "@/components/widgets/labels/UserInputLabel.vue";

const MenuList = defineComponent({
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
		virtualScrollingEntryHeight: { type: Number as PropType<number>, default: 0 },
		defaultAction: { type: Function as PropType<() => void>, required: false },
	},
	data() {
		return {
			isOpen: this.open,
			highlighted: this.activeEntry as MenuListEntry | undefined,
			virtualScrollingEntriesStart: 0,
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
			const flatEntries = this.entries.flat().filter((entry) => !entry.disabled);
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
			if (!this.virtualScrollingEntryHeight) return;
			this.virtualScrollingEntriesStart = (e.target as HTMLElement)?.scrollTop || 0;
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
		virtualScrollingTotalHeight() {
			return this.entries[0].length * this.virtualScrollingEntryHeight;
		},
		virtualScrollingStartIndex() {
			return Math.floor(this.virtualScrollingEntriesStart / this.virtualScrollingEntryHeight);
		},
		virtualScrollingEndIndex() {
			return Math.min(this.entries[0].length, this.virtualScrollingStartIndex + 1 + 400 / this.virtualScrollingEntryHeight);
		},
	},
	components: {
		FloatingMenu,
		Separator,
		IconLabel,
		UserInputLabel,
		LayoutRow,
		LayoutCol,
	},
});
export default MenuList;
</script>

<template>
	<FloatingMenu :class="'menu-list'" :direction="direction" :type="MenuType.Dropdown" ref="floatingMenu" :windowEdgeMargin="0" :scrollable="scrollable" data-hover-menu-keep-open>
		<template v-for="(section, sectionIndex) in menuEntries" :key="sectionIndex">
			<Separator :type="SeparatorType.List" :direction="SeparatorDirection.Vertical" v-if="sectionIndex > 0" />
			<div
				v-for="(entry, entryIndex) in section"
				:key="entryIndex"
				class="row"
				:class="{ open: isMenuEntryOpen(entry), active: entry === activeEntry }"
				@click="handleEntryClick(entry)"
				@mouseenter="handleEntryMouseEnter(entry)"
				@mouseleave="handleEntryMouseLeave(entry)"
				:data-hover-menu-spawner-extend="entry.children && []"
			>
				<CheckboxInput v-if="entry.checkbox" v-model:checked="entry.checked" :outlineStyle="true" :class="'entry-checkbox'" />
				<IconLabel v-else-if="entry.icon && drawIcon" :icon="entry.icon" :class="'entry-icon'" />
				<div v-else-if="drawIcon" class="no-icon" />

				<span class="entry-label">{{ entry.label }}</span>

				<IconLabel v-if="entry.shortcutRequiresLock && !fullscreen.keyboardLocked" :icon="'Info'" :title="keyboardLockInfoMessage" />
				<UserInputLabel v-else-if="entry.shortcut && entry.shortcut.length" :inputKeys="[entry.shortcut]" />

				<div class="submenu-arrow" v-if="entry.children && entry.children.length"></div>
				<div class="no-submenu-arrow" v-else></div>

				<MenuList
					v-if="entry.children"
					@click.stop
					:direction="MenuDirection.TopRight"
					:menuEntries="entry.children"
					v-bind="{ defaultAction, minWidth, drawIcon, scrollable }"
					:ref="(ref) => setEntryRefs(entry, ref)"
				/>
			</div>
		</template>
	</FloatingMenu>
</template>

<style lang="scss">
.menu-list {
	.floating-menu-container .floating-menu-content {
		padding: 4px 0;
		position: absolute;
		min-width: 100%;

		.row {
			height: 20px;
			display: flex;
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
				margin-left: 4px;
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
import { keyboardLockApiSupported } from "@/utilities/fullscreen";
import { SeparatorDirection, SeparatorType } from "@/components/widgets/widgets";

import FloatingMenu, { MenuDirection, MenuType } from "@/components/widgets/floating-menus/FloatingMenu.vue";
import Separator from "@/components/widgets/separators/Separator.vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";
import CheckboxInput from "@/components/widgets/inputs/CheckboxInput.vue";
import UserInputLabel from "@/components/widgets/labels/UserInputLabel.vue";

export type MenuListEntries = Array<MenuListEntry>;
export type SectionsOfMenuListEntries = Array<MenuListEntries>;

interface MenuListEntryData {
	value?: string;
	label?: string;
	icon?: string;
	checkbox?: boolean;
	shortcut?: Array<string>;
	shortcutRequiresLock?: boolean;
	action?: Function;
	children?: SectionsOfMenuListEntries;
}

export type MenuListEntry = MenuListEntryData & { ref?: typeof FloatingMenu | typeof MenuList; checked?: boolean };

const KEYBOARD_LOCK_USE_FULLSCREEN = "This hotkey is reserved by the browser, but becomes available in fullscreen mode";
const KEYBOARD_LOCK_SWITCH_BROWSER = "This hotkey is reserved by the browser, but becomes available in Chrome, Edge, and Opera which support the Keyboard.lock() API";

const MenuList = defineComponent({
	inject: ["fullscreen"],
	props: {
		direction: { type: String as PropType<MenuDirection>, default: MenuDirection.Bottom },
		menuEntries: { type: Array as PropType<SectionsOfMenuListEntries>, required: true },
		activeEntry: { type: Object as PropType<MenuListEntry>, required: false },
		defaultAction: { type: Function as PropType<Function | undefined>, required: false },
		minWidth: { type: Number, default: 0 },
		drawIcon: { type: Boolean, default: false },
		scrollable: { type: Boolean, default: false },
	},
	methods: {
		setEntryRefs(menuEntry: MenuListEntry, ref: typeof FloatingMenu) {
			if (ref) menuEntry.ref = ref;
		},
		handleEntryClick(menuEntry: MenuListEntry) {
			(this.$refs.floatingMenu as typeof FloatingMenu).setClosed();

			if (menuEntry.checkbox) menuEntry.checked = !menuEntry.checked;

			if (menuEntry.action) menuEntry.action();
			else if (this.defaultAction) this.defaultAction();

			this.$emit("update:activeEntry", menuEntry);
		},
		handleEntryMouseEnter(menuEntry: MenuListEntry) {
			if (!menuEntry.children || !menuEntry.children.length) return;

			if (menuEntry.ref) menuEntry.ref.setOpen();
			else throw new Error("The menu bar floating menu has no associated ref");
		},
		handleEntryMouseLeave(menuEntry: MenuListEntry) {
			if (!menuEntry.children || !menuEntry.children.length) return;

			if (menuEntry.ref) menuEntry.ref.setClosed();
			else throw new Error("The menu bar floating menu has no associated ref");
		},
		isMenuEntryOpen(menuEntry: MenuListEntry): boolean {
			if (!menuEntry.children || !menuEntry.children.length) return false;

			if (menuEntry.ref) return menuEntry.ref.isOpen();

			return false;
		},
		setOpen() {
			(this.$refs.floatingMenu as typeof FloatingMenu).setOpen();
		},
		setClosed() {
			(this.$refs.floatingMenu as typeof FloatingMenu).setClosed();
		},
		isOpen(): boolean {
			const floatingMenu = this.$refs.floatingMenu as typeof FloatingMenu;
			return Boolean(floatingMenu && floatingMenu.isOpen());
		},
		measureAndReportWidth() {
			// API is experimental but supported in all browsers - https://developer.mozilla.org/en-US/docs/Web/API/FontFaceSet
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			(document as any).fonts.ready.then(() => {
				const floatingMenu = this.$refs.floatingMenu as typeof FloatingMenu;

				// Save open/closed state before forcing open, if necessary, for measurement
				const initiallyOpen = floatingMenu.isOpen();
				if (!initiallyOpen) floatingMenu.setOpen();

				floatingMenu.disableMinWidth((initialMinWidth: string) => {
					floatingMenu.getWidth((width: number) => {
						floatingMenu.enableMinWidth(initialMinWidth);

						// Restore open/closed state if it was forced open for measurement
						if (!initiallyOpen) floatingMenu.setClosed();

						this.$emit("width-changed", width);
					});
				});
			});
		},
	},
	computed: {
		menuEntriesWithoutRefs(): Array<Array<MenuListEntryData>> {
			const { menuEntries } = this;
			return menuEntries.map((entries) =>
				entries.map((entry) => {
					// eslint-disable-next-line @typescript-eslint/no-unused-vars
					const { ref, ...entryWithoutRef } = entry;
					return entryWithoutRef;
				})
			);
		},
	},
	mounted() {
		this.measureAndReportWidth();
	},
	updated() {
		this.measureAndReportWidth();
	},
	watch: {
		menuEntriesWithoutRefs: {
			handler() {
				this.measureAndReportWidth();
			},
			deep: true,
		},
	},
	data() {
		return {
			keyboardLockInfoMessage: keyboardLockApiSupported() ? KEYBOARD_LOCK_USE_FULLSCREEN : KEYBOARD_LOCK_SWITCH_BROWSER,
			SeparatorDirection,
			SeparatorType,
			MenuDirection,
			MenuType,
		};
	},
	components: {
		FloatingMenu,
		Separator,
		IconLabel,
		CheckboxInput,
		UserInputLabel,
	},
});
export default MenuList;
</script>

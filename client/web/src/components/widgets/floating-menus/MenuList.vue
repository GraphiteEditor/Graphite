<template>
	<FloatingMenu :class="'menu-list'" :direction="direction" :type="MenuType.Dropdown" ref="floatingMenu" :windowEdgeMargin="0" data-hover-menu-keep-open>
		<template v-for="(section, sectionIndex) in menuEntries" :key="sectionIndex">
			<Separator :type="SeparatorType.List" :direction="SeparatorDirection.Vertical" v-if="sectionIndex > 0" />
			<div
				v-for="(entry, entryIndex) in section"
				:key="entryIndex"
				class="row"
				:class="{ open: isMenuEntryOpen(entry), active: entry === currentEntry }"
				@click="handleEntryClick(entry)"
				@mouseenter="handleEntryMouseEnter(entry)"
				@mouseleave="handleEntryMouseLeave(entry)"
				:data-hover-menu-spawner-extend="entry.children && []"
			>
				<Icon :icon="entry.icon" v-if="entry.icon && drawIcon" />
				<div class="no-icon" v-else-if="drawIcon" />
				<span class="entry-label">{{ entry.label }}</span>
				<UserInputLabel v-if="entry.shortcut && entry.shortcut.length" :inputKeys="[entry.shortcut]" />
				<div class="submenu-arrow" v-if="entry.children && entry.children.length"></div>
				<div class="no-submenu-arrow" v-else></div>
				<MenuList
					v-if="entry.children"
					:direction="MenuDirection.TopRight"
					:menuEntries="entry.children"
					v-model:active-entry="currentEntry"
					:minWidth="minWidth"
					:drawIcon="drawIcon"
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

			.icon svg {
				fill: var(--color-e-nearwhite);
			}

			.no-icon {
				width: 16px;
			}

			.entry-label {
				flex: 1 1 100%;
				margin-left: 8px;
			}

			.icon,
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
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";
import FloatingMenu, { MenuDirection, MenuType } from "./FloatingMenu.vue";
import Separator, { SeparatorDirection, SeparatorType } from "../Separator.vue";
import Icon from "../labels/Icon.vue";
import UserInputLabel from "../labels/UserInputLabel.vue";

export type MenuListEntries = Array<MenuListEntry>;
export type SectionsOfMenuListEntries = Array<MenuListEntries>;

interface MenuListEntryData {
	label?: string;
	icon?: string;
	// TODO: Add `checkbox` (which overrides any `icon`)
	shortcut?: Array<string>;
	action?: Function;
	children?: SectionsOfMenuListEntries;
}

export type MenuListEntry = MenuListEntryData & { ref?: typeof FloatingMenu | typeof MenuList };

const MenuList = defineComponent({
	props: {
		direction: { type: String as PropType<MenuDirection>, default: MenuDirection.Bottom },
		menuEntries: { type: Array as PropType<SectionsOfMenuListEntries>, required: true },
		activeEntry: { type: Object as PropType<MenuListEntry>, required: false },
		minWidth: { type: Number, default: 0 },
		drawIcon: { type: Boolean, default: false },
	},
	methods: {
		setEntryRefs(menuEntry: MenuListEntry, ref: typeof FloatingMenu) {
			if (ref) menuEntry.ref = ref;
		},
		handleEntryClick(menuEntry: MenuListEntry) {
			(this.$refs.floatingMenu as typeof FloatingMenu).setClosed();

			if (menuEntry.action) {
				menuEntry.action();
			} else {
				this.$emit("update:activeEntry", menuEntry);
			}
		},
		handleEntryMouseEnter(menuEntry: MenuListEntry) {
			if (!menuEntry.children || !menuEntry.children.length) return;

			if (menuEntry.ref) {
				menuEntry.ref.setOpen();
			} else throw new Error("The menu bar floating menu has no associated ref");
		},
		handleEntryMouseLeave(menuEntry: MenuListEntry) {
			if (!menuEntry.children || !menuEntry.children.length) return;

			if (menuEntry.ref) {
				menuEntry.ref.setClosed();
			} else throw new Error("The menu bar floating menu has no associated ref");
		},
		isMenuEntryOpen(menuEntry: MenuListEntry): boolean {
			if (!menuEntry.children || !menuEntry.children.length) return false;

			if (menuEntry.ref) {
				return menuEntry.ref.isOpen();
			}
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
			currentEntry: this.activeEntry,
			SeparatorDirection,
			SeparatorType,
			MenuDirection,
			MenuType,
		};
	},
	components: {
		FloatingMenu,
		Separator,
		Icon,
		UserInputLabel,
	},
});
export default MenuList;
</script>

<template>
	<FloatingMenu :class="'menu-list'" :direction="direction" :type="MenuType.Dropdown" ref="floatingMenu" :windowEdgeMargin="0" data-hover-menu-keep-open>
		<template v-for="(section, sectionIndex) in menuEntries" :key="sectionIndex">
			<Separator :type="SeparatorType.List" :direction="SeparatorDirection.Vertical" v-if="sectionIndex > 0" />
			<div
				v-for="(entry, entryIndex) in section"
				:key="entryIndex"
				class="row"
				:class="{ open: isMenuEntryOpen(entry) }"
				@click="handleEntryClick(entry)"
				@mouseenter="handleEntryMouseEnter(entry)"
				@mouseleave="handleEntryMouseLeave(entry)"
				:data-hover-menu-spawner-extend="entry.children && []"
			>
				<Icon :icon="entry.icon" v-if="entry.icon" />
				<div class="no-icon" v-else />
				<span class="label">{{ entry.label }}</span>
				<UserInputLabel v-if="entry.shortcut && entry.shortcut.length" :inputKeys="[entry.shortcut]" />
				<div class="submenu-arrow" v-if="entry.children && entry.children.length"></div>
				<div class="no-submenu-arrow" v-else></div>
				<MenuList v-if="entry.children" :menuEntries="entry.children" :direction="MenuDirection.TopRight" :ref="(ref) => setEntryRefs(entry, ref)" />
			</div>
		</template>
	</FloatingMenu>
</template>

<style lang="scss">
.menu-list {
	.floating-menu-container .floating-menu-content {
		min-width: 240px;
		padding: 4px 0;

		.row {
			height: 20px;
			display: flex;
			align-items: center;
			white-space: nowrap;
			position: relative;

			& > * {
				flex: 0 0 auto;
			}

			.icon svg {
				fill: var(--color-e-nearwhite);
			}

			.no-icon {
				width: 16px;
			}

			.label {
				flex: 1 1 100%;
			}

			.icon,
			.no-icon,
			.label {
				margin: 0 4px;
			}

			.user-input-label {
				margin: 0;
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
				margin-left: 4px;
				margin-right: 2px;
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
import { defineComponent, PropType } from "vue";
import FloatingMenu, { MenuDirection, MenuType } from "./FloatingMenu.vue";
import Separator, { SeparatorDirection, SeparatorType } from "../Separator.vue";
import Icon from "../labels/Icon.vue";
import UserInputLabel from "../labels/UserInputLabel.vue";

export type MenuListEntries = Array<MenuListEntry>;

export interface MenuListEntry {
	label?: string;
	icon?: string;
	// TODO: Add `checkbox` (which overrides any `icon`)
	shortcut?: Array<string>;
	action?: Function;
	children?: Array<Array<MenuListEntry>>;
	ref?: typeof FloatingMenu | typeof MenuList;
}
const MenuList = defineComponent({
	props: {
		direction: { type: String as PropType<MenuDirection>, value: MenuDirection.Bottom },
		menuEntries: { type: Array as PropType<MenuListEntries>, required: true },
	},
	methods: {
		setEntryRefs(menuEntry: MenuListEntry, ref: typeof FloatingMenu) {
			if (ref) menuEntry.ref = ref;
		},
		handleEntryClick(menuEntry: MenuListEntry) {
			if (menuEntry.action) menuEntry.action();
			else alert("This action is not yet implemented");
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
	},
	data() {
		return {
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

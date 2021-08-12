<template>
	<slot></slot>
</template>

<script lang="ts">
import { defineComponent, toRef, computed } from "vue";
import FloatingMenu from "@/components/widgets/floating-menus/FloatingMenu.vue";

/** Transparent component that groups together multiple togglable floating menus such that the user can easily move between different menus by hovering their mouse over other toggles. */
export default defineComponent({
	data() {
		return {
			openMenu: undefined as typeof FloatingMenu | undefined,
		};
	},
	provide() {
		return {
			openedMenuInGroup: toRef(this, "openMenu"),
			notifyGroupedFloatingMenuOpened: (newOpenMenu: typeof FloatingMenu) => {
				this.openMenu = newOpenMenu;
			},
			notifyGroupedFloatingMenuClosed: (closedMenu: typeof FloatingMenu) => {
				if (this.openMenu === closedMenu) {
					this.openMenu = undefined;
				}
			},
			openMenuOnHover: computed(() => this.openMenu !== undefined),
		};
	},
});
</script>

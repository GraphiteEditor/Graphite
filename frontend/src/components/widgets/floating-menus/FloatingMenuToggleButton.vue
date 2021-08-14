<template>
	<div class="toggle-button" @mousedown.stop="toggleFloatingMenu()" @mouseenter="maybeOpenOnHover()">
		<slot name="button" :isOpen="isOpen"></slot>
	</div>
	<slot
		name="popup"
		:isOpen="isOpen"
		:isOpenChanged="
			(newIsOpen) => {
				this.isOpen = newIsOpen;
			}
		"
	>
		<FloatingMenu v-bind="$props" v-model:isOpen="isOpen">
			<slot name="menu">Empty Menu</slot>
		</FloatingMenu>
	</slot>
</template>

<style lang="scss">
.toggle-button {
	display: contents;
}
</style>

<script lang="ts">
import { defineComponent, inject } from "vue";
import FloatingMenu, { MenuDirection } from "@/components/widgets/floating-menus/FloatingMenu.vue";

/**
 * A button that opens a popup menu when pressed.
 * Compatible with FloatingMenuToggleGroup such that when one popup in a group is opened,
 * others can be opened by hovering over the toggle instead of clicking it.
 *
 * Props:
 *   All props passed to this component are forwarded to FloatingMenu when the `menu` slot is used. The `type` prop should be considered mandatory in this case.
 *   When the `popup` slot is used instead, no props are necessary.
 *
 * Slots:
 *   button: Display the toggle button itself. Bind to slotProps.isOpen to modify the button style when the menu is opened.
 *   menu: Contents of the floating menu, contained within a FloatingMenu component. Use this one 90% of the time.
 *   popup: Only use this when the popup should not be a FloatingMenu (instead something like MenuList). Bind to slotProps.isOpen and slotProps.isOpenChanged.
 */
export default defineComponent({
	inheritAttrs: false,
	components: {
		FloatingMenu,
	},
	props: {
		direction: { type: String, default: MenuDirection.Bottom },
		disabled: { type: Boolean, default: false },
		type: { type: String, required: false },
		windowEdgeMargin: { type: Number, default: 8 },
		minWidth: { type: Number, default: 0 },
		scrollable: { type: Boolean, default: false },
	},
	methods: {
		toggleFloatingMenu() {
			this.isOpen = !this.disabled && !this.isOpen;
		},
		maybeOpenOnHover() {
			if (this.openOnHover) {
				this.isOpen = true;
			}
		},
	},
	setup() {
		return {
			openOnHover: inject("openMenuOnHover", false),
		};
	},
	data() {
		return {
			isOpen: false,
		};
	},
});
</script>

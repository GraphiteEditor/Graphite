<template>
	<span class="toggle-button" @mousedown.stop="toggleFloatingMenu()" @mouseenter="maybeOpenOnHover()">
		<slot name="button" :menuIsOpen="isOpen"></slot>
	</span>
	<FloatingMenu v-bind="$props" v-model:isOpen="isOpen">
		<slot name="menu">Empty Menu</slot>
	</FloatingMenu>
</template>

<script lang="ts">
import { defineComponent, inject } from "vue";
import FloatingMenu, { MenuDirection } from "@/components/widgets/floating-menus/FloatingMenu.vue";

export default defineComponent({
	inheritAttrs: false,
	components: {
		FloatingMenu,
	},
	props: {
		direction: { type: String, default: MenuDirection.Bottom },
		disabled: { type: Boolean, default: false },
		type: { type: String, required: true },
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
	setup(props) {
		return {
			...props,
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

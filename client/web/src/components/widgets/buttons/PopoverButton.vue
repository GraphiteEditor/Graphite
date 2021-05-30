<template>
	<div class="popover-button">
		<IconButton :icon="icon" :size="16" @click="clickButton" />
		<FloatingMenu :type="MenuType.Popover" :direction="MenuDirection.Bottom" ref="floatingMenu">
			<slot></slot>
		</FloatingMenu>
	</div>
</template>

<style lang="scss">
.popover-button {
	display: inline-block;
	position: relative;
	width: 16px;
	height: 24px;
	flex: 0 0 auto;

	.floating-menu {
		left: 50%;
	}

	.icon-button {
		width: 100%;
		height: 100%;
		padding: 0;
		outline: none;
		border: none;
		border-radius: 2px;
		vertical-align: top;
		background: var(--color-1-nearblack);
		fill: var(--color-e-nearwhite);

		&:hover {
			background: var(--color-6-lowergray);
			fill: var(--color-f-white);
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import IconButton from "./IconButton.vue";
import FloatingMenu, { MenuDirection, MenuType } from "../floating-menus/FloatingMenu.vue";

export enum PopoverButtonIcon {
	"DropdownArrow" = "DropdownArrow",
	"VerticalEllipsis" = "VerticalEllipsis",
}

export default defineComponent({
	components: {
		FloatingMenu,
		IconButton,
	},
	props: {
		icon: { type: String, default: PopoverButtonIcon.DropdownArrow },
	},
	methods: {
		clickButton() {
			(this.$refs.floatingMenu as typeof FloatingMenu).setOpen();
		},
	},
	data() {
		return {
			MenuDirection,
			MenuType,
		};
	},
});
</script>

<template>
	<div class="popover-button">
		<IconButton :icon="icon" :size="16" @click="clickButton" />
		<Popover :direction="PopoverDirection.Bottom" ref="popover">
			<slot></slot>
		</Popover>
	</div>
</template>

<style lang="scss">
.popover-button {
	display: inline-block;
	position: relative;
	width: 16px;
	height: 24px;
	flex: 0 0 auto;

	.popover {
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
import Popover, { PopoverDirection } from "../overlays/Popover.vue";

export enum PopoverButtonIcon {
	"DropdownArrow" = "DropdownArrow",
	"VerticalEllipsis" = "VerticalEllipsis",
}

export default defineComponent({
	components: {
		Popover,
		IconButton,
	},
	props: {
		icon: { type: String, default: PopoverButtonIcon.DropdownArrow },
	},
	methods: {
		clickButton() {
			(this.$refs.popover as typeof Popover).setOpen();
		},
	},
	data() {
		return {
			PopoverDirection,
		};
	},
});
</script>

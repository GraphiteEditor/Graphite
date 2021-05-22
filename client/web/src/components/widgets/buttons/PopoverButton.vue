<template>
	<div class="popover-button">
		<button @click="clickButton">
			<component :is="icon" />
		</button>
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
	margin: 2px 4px;

	.popover {
		left: 50%;
	}

	button {
		width: 100%;
		height: 100%;
		padding: 0;
		outline: none;
		border: none;
		border-radius: 2px;
		vertical-align: top;
		background: #111;
		fill: #ddd;

		&:hover {
			background: #666;
			fill: #fff;
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import DropdownArrow from "../../../../assets/svg/16x24-bounds-8x16-icon/dropdown-arrow.svg";
import VerticalEllipsis from "../../../../assets/svg/16x24-bounds-8x16-icon/vertical-ellipsis.svg";
import Popover, { PopoverDirection } from "../overlays/Popover.vue";

export enum PopoverButtonIcon {
	"DropdownArrow" = "DropdownArrow",
	"VerticalEllipsis" = "VerticalEllipsis",
}

export default defineComponent({
	components: {
		VerticalEllipsis,
		DropdownArrow,
		Popover,
		PopoverDirection,
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

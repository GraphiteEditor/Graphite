<template>
	<button class="dropdown-button">
		<component :is="icon" />
	</button>
</template>

<style lang="scss">
.dropdown-button {
	width: 16px;
	height: 24px;
	margin: 2px 4px;
	padding: 0;
	outline: none;
	border: none;
	border-radius: 2px;
	background: #111;
	fill: #ddd;

	&:hover {
		background: #555;
		fill: #fff;
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import DropdownArrow from "../../../assets/svg/16x24-bounds-8x16-icon/dropdown-arrow.svg";
import VerticalEllipsis from "../../../assets/svg/16x24-bounds-8x16-icon/vertical-ellipsis.svg";

export enum DropdownButtonIcon {
	"DropdownArrow" = "DropdownArrow",
	"VerticalEllipsis" = "VerticalEllipsis",
}

export default defineComponent({
	components: {
		VerticalEllipsis,
		DropdownArrow,
	},
	props: {
		icon: { type: String, default: DropdownButtonIcon.DropdownArrow },
	},
});
</script>

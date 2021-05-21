<template>
	<div class="working-colors">
		<div class="swatch-pair">
			<div class="secondary swatch">
				<button @click="clickSecondarySwatch" style="background: white"></button>
				<PopoverMount :direction="PopoverDirection.Right" horizontal ref="secondarySwatchPopover">
					<ColorPicker />
				</PopoverMount>
			</div>
			<div class="primary swatch">
				<button @click="clickPrimarySwatch" style="background: black"></button>
				<PopoverMount :direction="PopoverDirection.Right" horizontal ref="primarySwatchPopover">
					<ColorPicker />
				</PopoverMount>
			</div>
		</div>
		<div class="swap-and-reset">
			<IconButton :size="16">
				<SwapButton />
			</IconButton>
			<IconButton :size="16">
				<ResetColorsButton />
			</IconButton>
		</div>
	</div>
</template>

<style lang="scss">
.working-colors {
	.swatch-pair {
		display: flex;
		// Reversed order of elements paired with `column-reverse` allows primary to overlap secondary without relying on `z-index`
		flex-direction: column-reverse;
	}

	.swatch {
		width: 28px;
		height: 28px;
		margin: 2px;
		position: relative;

		button {
			width: 100%;
			height: 100%;
			border-radius: 50%;
			border: 2px #888 solid;
			box-shadow: 0 0 0 2px #333;
			margin: 0;
			padding: 0;
			box-sizing: border-box;
			outline: none;
		}

		.popover-mount {
			top: 50%;
			right: -2px;
		}

		&.primary {
			margin-bottom: -8px;
		}
	}

	.swap-and-reset {
		font-size: 0;
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import PopoverMount, { PopoverDirection } from "./PopoverMount.vue";
import ColorPicker from "../popovers/ColorPicker.vue";
import IconButton from "./IconButton.vue";
import SwapButton from "../../../assets/svg/16x16-bounds-12x12-icon/swap.svg";
import ResetColorsButton from "../../../assets/svg/16x16-bounds-12x12-icon/reset-colors.svg";

export default defineComponent({
	components: {
		PopoverMount,
		ColorPicker,
		IconButton,
		SwapButton,
		ResetColorsButton,
	},
	methods: {
		clickPrimarySwatch() {
			(this.$refs.primarySwatchPopover as typeof PopoverMount).setOpen();
			(this.$refs.secondarySwatchPopover as typeof PopoverMount).setClosed();
		},
		clickSecondarySwatch() {
			(this.$refs.secondarySwatchPopover as typeof PopoverMount).setOpen();
			(this.$refs.primarySwatchPopover as typeof PopoverMount).setClosed();
		},
	},
	data() {
		return {
			PopoverDirection,
		};
	},
});
</script>

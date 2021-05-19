<template>
	<div class="working-colors">
		<div class="swatch-pair">
			<button @click="clickSecondarySwatch" class="secondary swatch" style="background: white">
				<PopoverMount ref="secondarySwatchPopover">
					<ColorPicker />
				</PopoverMount>
			</button>
			<button @click="clickPrimarySwatch" class="primary swatch" style="background: black">
				<PopoverMount ref="primarySwatchPopover">
					<ColorPicker />
				</PopoverMount>
			</button>
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
		width: 24px;
		height: 24px;
		border-radius: 50%;
		border: 2px #888 solid;
		box-shadow: 0 0 0 2px #333;
		margin: 2px;
		padding: 0;
		box-sizing: unset;
		outline: none;
		position: relative;

		.popover-mount {
			right: -4px;
		}
	}

	.primary.swatch {
		margin-bottom: -8px;
	}

	.swap-and-reset {
		font-size: 0;
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import PopoverMount from "./PopoverMount.vue";
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
});
</script>

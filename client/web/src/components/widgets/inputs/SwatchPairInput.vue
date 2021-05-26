<template>
	<div class="swatch-pair">
		<div class="secondary swatch">
			<button @click="clickSecondarySwatch" ref="secondaryButton"></button>
			<Popover :direction="PopoverDirection.Right" horizontal ref="secondarySwatchPopover">
				<ColorPicker v-model:color="secondaryColor" />
			</Popover>
		</div>
		<div class="primary swatch">
			<button @click="clickPrimarySwatch" ref="primaryButton"></button>
			<Popover :direction="PopoverDirection.Right" horizontal ref="primarySwatchPopover">
				<ColorPicker v-model:color="primaryColor" />
			</Popover>
		</div>
	</div>
</template>

<style lang="scss">
.swatch-pair {
	display: flex;
	// Reversed order of elements paired with `column-reverse` allows primary to overlap secondary without relying on `z-index`
	flex-direction: column-reverse;

	.swatch {
		width: 28px;
		height: 28px;
		margin: 0 2px;
		position: relative;

		button {
			--swatch-color: white;
			width: 100%;
			height: 100%;
			border-radius: 50%;
			border: 2px var(--color-7-middlegray) solid;
			box-shadow: 0 0 0 2px var(--color-3-darkgray);
			margin: 0;
			padding: 0;
			box-sizing: border-box;
			outline: none;
			background: linear-gradient(45deg, #cccccc 25%, transparent 25%, transparent 75%, #cccccc 75%), linear-gradient(45deg, #cccccc 25%, transparent 25%, transparent 75%, #cccccc 75%),
				linear-gradient(#ffffff, #ffffff);
			background-size: 16px 16px;
			background-position: 0 0, 8px 8px;

			&::before {
				content: "";
				display: block;
				width: 100%;
				height: 100%;
				background: var(--swatch-color);
				background-size: 16px 16px;
				background-position: 0 0, 8px 8px;
				border-radius: 50%;
			}
		}

		.popover {
			top: 50%;
			right: -2px;
		}

		&.primary {
			margin-bottom: -8px;
		}
	}
}
</style>

<script lang="ts">
import { RGB2Floats } from "@/lib/utils";
import { defineComponent } from "vue";
import ColorPicker from "../../popovers/ColorPicker.vue";
import Popover, { PopoverDirection } from "../overlays/Popover.vue";

const wasm = import("../../../../wasm/pkg");

export default defineComponent({
	components: {
		Popover,
		ColorPicker,
	},
	props: {},
	methods: {
		clickPrimarySwatch() {
			(this.$refs.primarySwatchPopover as typeof Popover).setOpen();
			(this.$refs.secondarySwatchPopover as typeof Popover).setClosed();
		},
		clickSecondarySwatch() {
			(this.$refs.secondarySwatchPopover as typeof Popover).setOpen();
			(this.$refs.primarySwatchPopover as typeof Popover).setClosed();
		},
		async updatePrimaryColor() {
			const { update_primary_color, Color } = await wasm;

			let color = this.primaryColor;
			const button = this.$refs.primaryButton as HTMLButtonElement;
			button.style.setProperty("--swatch-color", `rgba(${color.r}, ${color.g}, ${color.b}, ${color.a})`);

			color = RGB2Floats(this.primaryColor);
			update_primary_color(new Color(color.r, color.g, color.b, color.a));
		},
		async updateSecondaryColor() {
			const { update_secondary_color, Color } = await wasm;

			let color = this.secondaryColor;
			const button = this.$refs.secondaryButton as HTMLButtonElement;
			button.style.setProperty("--swatch-color", `rgba(${color.r}, ${color.g}, ${color.b}, ${color.a})`);

			color = RGB2Floats(this.secondaryColor);
			update_secondary_color(new Color(color.r, color.g, color.b, color.a));
		},
	},
	data() {
		return {
			PopoverDirection,
			primaryColor: {
				r: 255,
				g: 255,
				b: 255,
				a: 1,
			},
			secondaryColor: {
				r: 0,
				g: 0,
				b: 0,
				a: 1,
			},
		};
	},
	mounted() {
		this.$watch("primaryColor", this.updatePrimaryColor, { immediate: true });
		this.$watch("secondaryColor", this.updateSecondaryColor, { immediate: true });
	},
});
</script>

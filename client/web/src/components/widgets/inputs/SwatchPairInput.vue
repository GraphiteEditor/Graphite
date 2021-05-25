<template>
	<div class="swatch-pair">
		<div class="secondary swatch">
			<button @click="clickSecondarySwatch" :style="{ background: primaryCSSColor }"></button>
			<Popover :direction="PopoverDirection.Right" horizontal ref="secondarySwatchPopover">
				<ColorPicker v-model:color="primaryColor" />
			</Popover>
		</div>
		<div class="primary swatch">
			<button @click="clickPrimarySwatch" :style="{ background: secondaryCSSColor }"></button>
			<Popover :direction="PopoverDirection.Right" horizontal ref="primarySwatchPopover">
				<ColorPicker v-model:color="secondaryColor" />
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
			width: 100%;
			height: 100%;
			border-radius: 50%;
			border: 2px var(--color-7-middlegray) solid;
			box-shadow: 0 0 0 2px var(--color-3-darkgray);
			margin: 0;
			padding: 0;
			box-sizing: border-box;
			outline: none;
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
import { handleResponse, ResponseType } from "@/response-handler";
import { defineComponent } from "vue";
import ColorPicker from "../../popovers/ColorPicker.vue";
import Popover, { PopoverDirection } from "../overlays/Popover.vue";

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
	computed: {
		primaryCSSColor() {
			// eslint-disable-next-line @typescript-eslint/ban-ts-ignore
			// @ts-ignore
			return `rgba(${this.primaryColor.r}, ${this.primaryColor.g}, ${this.primaryColor.b}, ${this.primaryColor.a})`;
		},
		secondaryCSSColor() {
			// eslint-disable-next-line @typescript-eslint/ban-ts-ignore
			// @ts-ignore
			return `rgba(${this.secondaryColor.r}, ${this.secondaryColor.g}, ${this.secondaryColor.b}, ${this.secondaryColor.a})`;
		},
	},
	mounted() {
		this.$watch("primaryColor", () => {
			handleResponse(ResponseType.UpdatePrimaryColor, {
				UpdatePrimaryColor: this.primaryColor,
			});
		});
		this.$watch("secondaryColor", () => {
			handleResponse(ResponseType.UpdateSecondaryColor, {
				UpdateSecondaryColor: this.secondaryColor,
			});
		});
	},
});
</script>

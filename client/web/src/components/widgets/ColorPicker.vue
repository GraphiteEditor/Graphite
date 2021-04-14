<template>
	<div class="color-picker"></div>
</template>

<style lang="scss">
.color-picker {
	&__picker {
		display: grid;
		grid-auto-flow: column;
		grid-gap: 10px;
		gap: 10px;
	}

	&__hue {
		position: relative;
		background: linear-gradient(
			rgb(255, 0, 0) 0%,
			rgb(255, 0, 255) 17%,
			rgb(0, 0, 255) 34%,
			rgb(0, 255, 255) 50%,
			rgb(0, 255, 0) 67%,
			rgb(255, 255, 0) 84%,
			rgb(255, 0, 0) 100%
		);
	}

	&__hue-selector {
		position: absolute;
		background: white;
		border-bottom: 1px solid black;
		right: -3px;
		width: 10px;
		height: 2px;
	}

	&__saturation {
		position: relative;
	}

	&__saturation-selector {
		border: 2px solid white;
		position: absolute;
		width: 14px;
		height: 14px;
		background: white;
		border-radius: 10px;
		top: -7px;
		left: -7px;
		box-sizing: border-box;
		z-index: 10;
	}

	&__brightness {
		width: 100%;
		height: 100%;
		background: linear-gradient(rgba(255, 255, 255, 0), rgba(0, 0, 0, 1));
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import { ColorPicker } from "../../lib/color-picker";

export default defineComponent({
	props: {
		color: {
			type: Object,
			default: () => ({ r: 0, g: 0, b: 0 }),
		},
	},

	data() {
		return {
			hex: "#000000",
			rgb: {
				r: 0,
				g: 0,
				b: 0,
			},
			hsv: {
				h: 0,
				s: 0,
				v: 0,
			},
		};
	},

	mounted() {
		const picker = new ColorPicker({
			el: this.$el as Element,
		});

		// @ts-ignore
		this.picker = picker;

		this.updateColor();

		picker.onChange(() => {
			const rgb = picker.getFloats();
			this.color.r = rgb.r;
			this.color.g = rgb.g;
			this.color.b = rgb.b;
		});
	},

	updated() {
		this.updateColor();
	},

	unmounted() {
		// @ts-ignore
		const picker = this.picker as ColorPicker;
		picker.dispose();
	},

	methods: {
		updateColor() {
			// @ts-ignore
			const picker = this.picker as ColorPicker;
			picker.setFloats(this.color.r, this.color.g, this.color.b);
			this.hex = `#${picker.getHexString()}`;
			const { r, g, b } = picker.getFloats();
			this.rgb.r = r;
			this.rgb.g = g;
			this.rgb.b = b;
			const { h, s, v } = picker.getHSV();
			this.hsv.h = h;
			this.hsv.s = s;
			this.hsv.v = v;
		},
	},
});
</script>

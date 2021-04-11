<template>
	<div class="color-picker"></div>
</template>

<style lang="scss">
</style>

<script lang="ts">
import { defineComponent } from "vue";
import ColorPicker from "simple-color-picker";

export type RGBAColor = {
	r: number;
	g: number;
	b: number;
	a?: number;
};

function rgbToHex(color: RGBAColor): string {
	const { r, g, b } = color;
	const hex = [
		"#",
		Math.round(r * 255)
			.toString(16)
			.padStart(2, "0"),
		Math.round(g * 255)
			.toString(16)
			.padStart(2, "0"),
		Math.round(b * 255)
			.toString(16)
			.padStart(2, "0")
	];

	return hex.join("").toUpperCase();
}

export default defineComponent({
	props: {
		color: {
			type: Object,
			default: () => {
				return { r: 0, g: 0, b: 0 };
			}
		}
	},

	mounted() {
		// @ts-ignore
		const picker = (this.picker = new ColorPicker({
			el: this.$el
		}));

		// @ts-ignore
		picker.setColor(rgbToHex(this.color));
		picker.onChange(() => {
			const rgb = picker.getRGB();
			this.color.r = rgb.r;
			this.color.g = rgb.g;
			this.color.b = rgb.b;
		});
	},

	updated() {
		// @ts-ignore
		this.picker.setColor(rgbToHex(this.color));
	},

	unmounted() {
		// @ts-ignore
		this.picker.remove();
	}
});
</script>
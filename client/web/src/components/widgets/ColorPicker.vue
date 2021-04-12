<template>
	<div class="color-picker"></div>
</template>

<style lang="scss">
</style>

<script lang="ts">
import { defineComponent } from "vue";
import { ColorPicker, RGBAColor } from "../../lib/ColorPicker";

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

		picker.setColor(this.color as RGBAColor);
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
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
		background: linear-gradient(rgb(255, 0, 0) 0%, rgb(255, 0, 255) 17%, rgb(0, 0, 255) 34%, rgb(0, 255, 255) 50%, rgb(0, 255, 0) 67%, rgb(255, 255, 0) 84%, rgb(255, 0, 0) 100%);
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
import ColorPicker from "../../lib/color-picker";

export default defineComponent({
	picker: new ColorPicker(),

	props: {
		red: {
			type: Number,
			default: 0,
		},
		green: {
			type: Number,
			default: 0,
		},
		blue: {
			type: Number,
			default: 0,
		},
		alpha: {
			type: Number,
			default: 1,
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
		const { picker } = this.$options as { picker: ColorPicker };
		picker.setParent(this.$el);

		this.updateColor();

		picker.onChange(() => {
			const rgb = picker.getFloats();
			this.$emit("update:red", rgb.r);
			this.$emit("update:green", rgb.g);
			this.$emit("update:blue", rgb.b);
			this.$emit("update:alpha", rgb.a);
		});
	},

	updated() {
		this.updateColor();
	},

	unmounted() {
		const { picker } = this.$options as { picker: ColorPicker };
		picker.dispose();
	},

	methods: {
		updateColor() {
			const { picker } = this.$options as { picker: ColorPicker };
			picker.setFloats(this.red, this.green, this.blue);
			picker.setAlpha(this.alpha);
		},
	},
});
</script>

<template>
	<div class="swatch-pair">
		<div class="secondary swatch">
			<button @click="() => clickSecondarySwatch()" ref="secondaryButton" data-hover-menu-spawner></button>
			<FloatingMenu :type="'Popover'" :direction="'Right'" horizontal ref="secondarySwatchFloatingMenu">
				<ColorPicker @update:color="(color) => secondaryColorChanged(color)" :color="secondaryColor" />
			</FloatingMenu>
		</div>
		<div class="primary swatch">
			<button @click="() => clickPrimarySwatch()" ref="primaryButton" data-hover-menu-spawner></button>
			<FloatingMenu :type="'Popover'" :direction="'Right'" horizontal ref="primarySwatchFloatingMenu">
				<ColorPicker @update:color="(color) => primaryColorChanged(color)" :color="primaryColor" />
			</FloatingMenu>
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
			--swatch-color: #ffffff;
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
			overflow: hidden;

			&::before {
				content: "";
				display: block;
				width: 100%;
				height: 100%;
				background: var(--swatch-color);
			}
		}

		.floating-menu {
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
import { defineComponent } from "vue";

import { RGBA, UpdateWorkingColors } from "@/dispatcher/js-messages";
import { rgbaToDecimalRgba } from "@/utilities/color";

import ColorPicker from "@/components/widgets/floating-menus/ColorPicker.vue";
import FloatingMenu from "@/components/widgets/floating-menus/FloatingMenu.vue";

export default defineComponent({
	inject: ["editor"],
	components: {
		FloatingMenu,
		ColorPicker,
	},
	methods: {
		clickPrimarySwatch() {
			(this.$refs.primarySwatchFloatingMenu as typeof FloatingMenu).setOpen();
			(this.$refs.secondarySwatchFloatingMenu as typeof FloatingMenu).setClosed();
		},
		clickSecondarySwatch() {
			(this.$refs.secondarySwatchFloatingMenu as typeof FloatingMenu).setOpen();
			(this.$refs.primarySwatchFloatingMenu as typeof FloatingMenu).setClosed();
		},
		primaryColorChanged(color: RGBA) {
			this.primaryColor = color;
			this.updatePrimaryColor();
		},
		secondaryColorChanged(color: RGBA) {
			this.secondaryColor = color;
			this.updateSecondaryColor();
		},
		async updatePrimaryColor() {
			let color = this.primaryColor;
			const button = this.$refs.primaryButton as HTMLButtonElement;
			button.style.setProperty("--swatch-color", `rgba(${color.r}, ${color.g}, ${color.b}, ${color.a})`);

			color = rgbaToDecimalRgba(this.primaryColor);
			this.editor.instance.update_primary_color(color.r, color.g, color.b, color.a);
		},
		async updateSecondaryColor() {
			let color = this.secondaryColor;
			const button = this.$refs.secondaryButton as HTMLButtonElement;
			button.style.setProperty("--swatch-color", `rgba(${color.r}, ${color.g}, ${color.b}, ${color.a})`);

			color = rgbaToDecimalRgba(this.secondaryColor);
			this.editor.instance.update_secondary_color(color.r, color.g, color.b, color.a);
		},
	},
	data() {
		return {
			primaryColor: { r: 0, g: 0, b: 0, a: 1 } as RGBA,
			secondaryColor: { r: 255, g: 255, b: 255, a: 1 } as RGBA,
		};
	},
	mounted() {
		this.editor.dispatcher.subscribeJsMessage(UpdateWorkingColors, (updateWorkingColors) => {
			this.primaryColor = updateWorkingColors.primary.toRgba();
			this.secondaryColor = updateWorkingColors.secondary.toRgba();

			const primaryButton = this.$refs.primaryButton as HTMLButtonElement;
			primaryButton.style.setProperty("--swatch-color", updateWorkingColors.primary.toRgbaCSS());

			const secondaryButton = this.$refs.secondaryButton as HTMLButtonElement;
			secondaryButton.style.setProperty("--swatch-color", updateWorkingColors.secondary.toRgbaCSS());
		});

		this.updatePrimaryColor();
		this.updateSecondaryColor();
	},
});
</script>

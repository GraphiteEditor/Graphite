<template>
	<div class="swatch-pair">
		<div class="secondary swatch">
			<button @click="clickSecondarySwatch" ref="secondaryButton" data-hover-menu-spawner></button>
			<FloatingMenu :type="MenuType.Popover" :direction="MenuDirection.Right" horizontal ref="secondarySwatchFloatingMenu">
				<ColorPicker v-model:color="secondaryColor" />
			</FloatingMenu>
		</div>
		<div class="primary swatch">
			<button @click="clickPrimarySwatch" ref="primaryButton" data-hover-menu-spawner></button>
			<FloatingMenu :type="MenuType.Popover" :direction="MenuDirection.Right" horizontal ref="primarySwatchFloatingMenu">
				<ColorPicker v-model:color="primaryColor" />
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
import { rgbToDecimalRgb } from "@/utilities/color";
import { defineComponent } from "vue";
import ColorPicker from "../floating-menus/ColorPicker.vue";
import FloatingMenu, { MenuDirection, MenuType } from "../floating-menus/FloatingMenu.vue";
import { ResponseType, registerResponseHandler, Response, UpdateWorkingColors } from "../../../utilities/response-handler";

const wasm = import("@/../wasm/pkg");

export default defineComponent({
	components: {
		FloatingMenu,
		ColorPicker,
	},
	props: {},
	methods: {
		clickPrimarySwatch() {
			this.getRef<typeof FloatingMenu>("primarySwatchFloatingMenu").setOpen();
			this.getRef<typeof FloatingMenu>("secondarySwatchFloatingMenu").setClosed();
		},

		clickSecondarySwatch() {
			this.getRef<typeof FloatingMenu>("secondarySwatchFloatingMenu").setOpen();
			this.getRef<typeof FloatingMenu>("primarySwatchFloatingMenu").setClosed();
		},

		getRef<T>(name: string) {
			return this.$refs[name] as T;
		},

		async updatePrimaryColor() {
			const { update_primary_color, Color } = await wasm;

			let color = this.primaryColor;
			const button = this.getRef<HTMLButtonElement>("primaryButton");
			button.style.setProperty("--swatch-color", `rgba(${color.r}, ${color.g}, ${color.b}, ${color.a})`);

			color = rgbToDecimalRgb(this.primaryColor);
			update_primary_color(new Color(color.r, color.g, color.b, color.a));
		},

		async updateSecondaryColor() {
			const { update_secondary_color, Color } = await wasm;

			let color = this.secondaryColor;
			const button = this.getRef<HTMLButtonElement>("secondaryButton");
			button.style.setProperty("--swatch-color", `rgba(${color.r}, ${color.g}, ${color.b}, ${color.a})`);

			color = rgbToDecimalRgb(this.secondaryColor);
			update_secondary_color(new Color(color.r, color.g, color.b, color.a));
		},
	},
	data() {
		return {
			MenuDirection,
			MenuType,
			primaryColor: { r: 0, g: 0, b: 0, a: 1 },
			secondaryColor: { r: 255, g: 255, b: 255, a: 1 },
		};
	},
	mounted() {
		this.$watch("primaryColor", this.updatePrimaryColor, { immediate: true });
		this.$watch("secondaryColor", this.updateSecondaryColor, { immediate: true });

		registerResponseHandler(ResponseType.UpdateWorkingColors, (responseData: Response) => {
			const colorData = responseData as UpdateWorkingColors;
			if (!colorData) return;
			const { primary, secondary } = colorData;
			this.primaryColor = { r: primary.red, g: primary.green, b: primary.blue, a: primary.alpha };
			this.secondaryColor = { r: secondary.red, g: secondary.green, b: secondary.blue, a: secondary.alpha };
		});
	},
});
</script>

<template>
	<LayoutRow class="color-input">
		<TextInput :value="value" :label="label" :disabled="disabled" @commitText="(value: string) => colorTextUpdated(value)" :center="true" />
		<Separator />
		<LayoutRow class="swatch">
			<button class="swatch-button" @click="() => menuOpen()" ref="colorSwatch"></button>
			<FloatingMenu :type="'Popover'" :direction="'Bottom'" horizontal ref="colorFloatingMenu" :windowEdgeMargin="40">
				<ColorPicker @update:color="(color) => colorSelectedInMenu(color)" :color="color" />
			</FloatingMenu>
		</LayoutRow>
		<!-- <ColorPicker :color="{ r: 0, g: 0, b: 0, a: 1 }" /> -->
	</LayoutRow>
</template>

<style lang="scss">
.color-input {
	.swatch-button {
		box-sizing: border-box;
		height: 24px;
		width: 24px;
		padding: 0;
		border: none;
		border-radius: 2px;
		--swatch-color: #ffffff;
		background-color: var(--swatch-color);
	}

	.floating-menu {
		margin-top: 24px;
	}

	.swatch {
		flex-grow: 0;
	}

	.text-input {
		input {
			text-align: center;
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { RGBA } from "@/dispatcher/js-messages";

import LayoutRow from "@/components/layout/LayoutRow.vue";
import ColorPicker from "@/components/widgets/floating-menus/ColorPicker.vue";
import FloatingMenu from "@/components/widgets/floating-menus/FloatingMenu.vue";
import TextInput from "@/components/widgets/inputs/TextInput.vue";
import Separator from "@/components/widgets/separators/Separator.vue";

export default defineComponent({
	emits: ["update:value"],
	props: {
		value: { type: String as PropType<string>, required: true },
		label: { type: String as PropType<string>, required: false },
		disabled: { type: Boolean as PropType<boolean>, default: false },
	},
	watch: {
		value(newValue: string) {
			// TODO: validation
			this.color = this.hexToRGB(newValue);
			this.updateButtonColor();
		},
	},
	data() {
		return {
			color: {} as RGBA,
		};
	},
	mounted() {
		this.color = this.hexToRGB(this.value);
		this.updateButtonColor();
	},
	methods: {
		menuOpen() {
			(this.$refs.colorFloatingMenu as typeof FloatingMenu).setOpen();
		},
		colorSelectedInMenu(color: RGBA) {
			const twoDigitHex = (val: number): string => val.toString(16).padStart(2, "0");
			const newValue = `${twoDigitHex(color.r)}${twoDigitHex(color.g)}${twoDigitHex(color.b)}${twoDigitHex(color.a * 255)}`;
			this.$emit("update:value", newValue);
		},
		colorTextUpdated(newValue: string) {
			this.$emit("update:value", newValue);
		},
		hexToRGB(hex: string) {
			const r = parseInt(hex.slice(0, 2), 16);
			const g = parseInt(hex.slice(2, 4), 16);
			const b = parseInt(hex.slice(4, 6), 16);
			const a = parseInt(hex.slice(6, 8), 16);
			return { r, g, b, a };
		},
		updateButtonColor() {
			const button = this.$refs.colorSwatch as HTMLButtonElement;
			button.style.setProperty("--swatch-color", `rgba(${this.color.r}, ${this.color.g}, ${this.color.b}, ${this.color.a})`);
		},
	},
	components: {
		TextInput,
		ColorPicker,
		LayoutRow,
		FloatingMenu,
		Separator,
	},
});
</script>

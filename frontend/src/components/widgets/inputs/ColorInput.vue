<template>
	<LayoutRow class="color-input">
		<TextInput :value="displayValue" :label="label" :disabled="disabled" @commitText="(value: string) => textInputUpdated(value)" :center="true" />
		<Separator :type="'Related'" />
		<LayoutRow class="swatch">
			<button class="swatch-button" @click="() => menuOpen()" :style="`--swatch-color: #${value}`"></button>
			<FloatingMenu :type="'Popover'" :direction="'Bottom'" horizontal ref="colorFloatingMenu">
				<ColorPicker @update:color="(color) => colorPickerUpdated(color)" :color="color" />
			</FloatingMenu>
		</LayoutRow>
	</LayoutRow>
</template>

<style lang="scss">
.color-input {
	.text-input input {
		text-align: center;
	}

	.swatch {
		flex: 0 0 auto;
		position: relative;

		.swatch-button {
			--swatch-color: #ffffff;
			height: 24px;
			width: 24px;
			bottom: 0;
			left: 50%;
			padding: 0;
			outline: none;
			border: none;
			border-radius: 2px;
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
			margin-top: 24px;
			left: 50%;
			bottom: 0;
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
	computed: {
		color() {
			const r = parseInt(this.value.slice(0, 2), 16);
			const g = parseInt(this.value.slice(2, 4), 16);
			const b = parseInt(this.value.slice(4, 6), 16);
			const a = parseInt(this.value.slice(6, 8), 16);
			return { r, g, b, a: a / 255 };
		},
		displayValue() {
			const value = this.value.toLowerCase();
			const shortenedIfOpaque = value.slice(-2) === "ff" ? value.slice(0, 6) : value;
			return `#${shortenedIfOpaque}`;
		},
	},
	methods: {
		colorPickerUpdated(color: RGBA) {
			const twoDigitHex = (value: number): string => value.toString(16).padStart(2, "0");
			const alphaU8Scale = Math.floor(color.a * 255);
			const newValue = `${twoDigitHex(color.r)}${twoDigitHex(color.g)}${twoDigitHex(color.b)}${twoDigitHex(alphaU8Scale)}`;
			this.$emit("update:value", newValue);
		},
		textInputUpdated(newValue: string) {
			const sanitizedMatch = newValue.match(/^\s*#?([0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{3})\s*$/);
			if (!sanitizedMatch) return;

			let sanitized;
			const match = sanitizedMatch[1];
			if (match.length === 3) {
				sanitized = match
					.split("")
					.map((byte) => `${byte}${byte}`)
					.concat("ff")
					.join("");
			} else if (match.length === 6) sanitized = `${match}ff`;
			else if (match.length === 8) sanitized = match;
			else return;

			this.$emit("update:value", sanitized);
		},
		menuOpen() {
			(this.$refs.colorFloatingMenu as typeof FloatingMenu).setOpen();
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

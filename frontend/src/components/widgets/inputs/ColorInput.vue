<template>
	<LayoutRow class="color-input" :title="tooltip">
		<OptionalInput v-if="!noTransparency" :icon="'CloseX'" :checked="Boolean(value)" @update:checked="(state: boolean) => updateEnabled(state)"></OptionalInput>
		<TextInput :value="displayValue" :label="label" :disabled="disabled || !value" @commitText="(value: string) => textInputUpdated(value)" :center="true" />
		<Separator :type="'Related'" />
		<LayoutRow class="swatch">
			<button class="swatch-button" :class="{ 'disabled-swatch': !value }" :style="`--swatch-color: #${value}`" @click="() => $emit('update:open', true)"></button>
			<FloatingMenu v-model:open="isOpen" :type="'Popover'" :direction="'Bottom'">
				<ColorPicker @update:color="(color: RGBA) => colorPickerUpdated(color)" :color="color" />
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

			&.disabled-swatch::after {
				content: "";
				position: absolute;
				border-top: 4px solid red;
				width: 33px;
				left: 22px;
				top: -4px;
				transform: rotate(135deg);
				transform-origin: 0% 100%;
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

import { RGBA } from "@/wasm-communication/messages";

import ColorPicker from "@/components/floating-menus/ColorPicker.vue";
import FloatingMenu from "@/components/floating-menus/FloatingMenu.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";
import OptionalInput from "@/components/widgets/inputs/OptionalInput.vue";
import TextInput from "@/components/widgets/inputs/TextInput.vue";
import Separator from "@/components/widgets/labels/Separator.vue";

export default defineComponent({
	emits: ["update:value", "update:open"],
	props: {
		value: { type: String as PropType<string | undefined>, required: false },
		label: { type: String as PropType<string>, required: false },
		noTransparency: { type: Boolean as PropType<boolean>, default: false },
		disabled: { type: Boolean as PropType<boolean>, default: false },
		tooltip: { type: String as PropType<string | undefined>, required: false },

		// Bound through `v-model`
		// TODO: See if this should be made to follow the pattern of DropdownInput.vue so this could be removed
		open: { type: Boolean as PropType<boolean>, required: true },
	},
	data() {
		return {
			isOpen: false,
		};
	},
	computed: {
		color() {
			if (!this.value) return { r: 0, g: 0, b: 0, a: 1 };

			const r = parseInt(this.value.slice(0, 2), 16);
			const g = parseInt(this.value.slice(2, 4), 16);
			const b = parseInt(this.value.slice(4, 6), 16);
			const a = parseInt(this.value.slice(6, 8), 16);
			return { r, g, b, a: a / 255 };
		},
		displayValue() {
			if (!this.value) return "";

			const value = this.value.toLowerCase();
			const shortenedIfOpaque = value.slice(-2) === "ff" ? value.slice(0, 6) : value;
			return `#${shortenedIfOpaque}`;
		},
	},
	watch: {
		// Called only when `open` is changed from outside this component (with v-model)
		open(newOpen: boolean) {
			this.isOpen = newOpen;
		},
		isOpen(newIsOpen: boolean) {
			this.$emit("update:open", newIsOpen);
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
			} else if (match.length === 6) {
				sanitized = `${match}ff`;
			} else if (match.length === 8) {
				sanitized = match;
			} else {
				return;
			}

			this.$emit("update:value", sanitized);
		},
		updateEnabled(value: boolean) {
			if (value) this.$emit("update:value", "000000");
			else this.$emit("update:value", undefined);
		},
	},
	components: {
		TextInput,
		ColorPicker,
		LayoutRow,
		FloatingMenu,
		Separator,
		OptionalInput,
	},
});
</script>

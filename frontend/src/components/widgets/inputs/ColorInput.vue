<template>
	<LayoutRow class="color-input">
		<TextInput :value="value" :label="label" :disabled="disabled" @commitText="(value: string) => textInputUpdated(value)" :center="true" />
		<Separator :type="'Related'" />
		<LayoutRow class="swatch">
			<button class="swatch-button" @click="() => menuOpen()" ref="colorSwatch" v-bind:style="{ background: `#${value}` }"></button>
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
		flex-grow: 0;
		position: relative;

		.swatch-button {
			box-sizing: border-box;
			height: 24px;
			width: 24px;
			padding: 0;
			outline: none;
			border: none;
			border-radius: 2px;
			bottom: 0;
			left: 50%;
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
	},
	methods: {
		colorPickerUpdated(color: RGBA) {
			const twoDigitHex = (val: number): string => val.toString(16).padStart(2, "0");
			const alphaU8Scale = Math.floor(color.a * 255);
			const newValue = `${twoDigitHex(color.r)}${twoDigitHex(color.g)}${twoDigitHex(color.b)}${twoDigitHex(alphaU8Scale)}`;
			this.$emit("update:value", newValue);
		},
		textInputUpdated(newValue: string) {
			if ((newValue.length !== 6 && newValue.length !== 8) || !newValue.match(/[A-F,a-f,0-9]*/)) {
				return;
			}
			this.$emit("update:value", newValue);
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

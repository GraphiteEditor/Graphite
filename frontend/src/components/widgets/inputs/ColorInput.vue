<template>
	<LayoutRow class="color-input" :title="tooltip">
		<button :class="{ none: value.none }" :style="{ '--color': value.toHexOptionalAlpha() }" @click="() => $emit('update:open', true)"></button>
		<ColorPicker v-model:open="isOpen" :color="value" @update:color="(color: Color) => colorPickerUpdated(color)" :allowNone="true" />
	</LayoutRow>
</template>

<style lang="scss">
.color-input {
	box-sizing: border-box;
	position: relative;
	border: 1px solid var(--color-7-middlegray);
	border-radius: 2px;
	padding: 1px;

	> button {
		position: relative;
		overflow: hidden;
		outline: none;
		border: none;
		padding: 0;
		margin: 0;
		width: 100%;
		height: 100%;
		border-radius: 1px;

		&::before {
			content: "";
			position: absolute;
			width: 100%;
			height: 100%;
			padding: 2px;
			top: -2px;
			left: -2px;
			background: linear-gradient(var(--color), var(--color)), var(--transparent-checkered-background);
			background-size: var(--transparent-checkered-background-size);
			background-position: var(--transparent-checkered-background-position);
		}

		&.none {
			background: var(--color-none);
			background-repeat: var(--color-none-repeat);
			background-position: var(--color-none-position);
			background-size: var(--color-none-size-24px);
			background-image: var(--color-none-image-24px);
		}
	}

	> .floating-menu {
		left: 50%;
		bottom: 0;
	}
}
</style>

<script lang="ts">
import { defineComponent, type PropType } from "vue";

import { Color } from "@/wasm-communication/messages";

import ColorPicker from "@/components/floating-menus/ColorPicker.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";

export default defineComponent({
	emits: ["update:value", "update:open"],
	props: {
		value: { type: Color as PropType<Color>, required: true },
		noTransparency: { type: Boolean as PropType<boolean>, default: false }, // TODO: Rename to allowTransparency, also implement allowNone
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
		colorPickerUpdated(color: Color) {
			this.$emit("update:value", color);
		},
	},
	components: {
		ColorPicker,
		LayoutRow,
	},
});
</script>

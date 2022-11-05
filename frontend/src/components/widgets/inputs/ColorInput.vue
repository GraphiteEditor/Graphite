<template>
	<LayoutRow class="color-input" :class="{ 'sharp-right-corners': sharpRightCorners }" :title="tooltip">
		<button
			:class="{ none: value.none, 'sharp-right-corners': sharpRightCorners }"
			:style="{ '--chosen-color': value.toHexOptionalAlpha() }"
			@click="() => $emit('update:open', true)"
			tabindex="0"
			data-floating-menu-spawner
		>
			<TextLabel :bold="true" class="chip" v-if="chip">{{ chip }}</TextLabel>
		</button>
		<ColorPicker v-model:open="isOpen" :color="value" @update:color="(color: Color) => colorPickerUpdated(color)" :allowNone="true" />
	</LayoutRow>
</template>

<style lang="scss">
.color-input {
	box-sizing: border-box;
	position: relative;
	border: 1px solid var(--color-5-dullgray);
	border-radius: 2px;
	padding: 1px;

	> button {
		position: relative;
		overflow: hidden;
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
			background: linear-gradient(var(--chosen-color), var(--chosen-color)), var(--color-transparent-checkered-background);
			background-size: var(--color-transparent-checkered-background-size);
			background-position: var(--color-transparent-checkered-background-position);
		}

		&.none {
			background: var(--color-none);
			background-repeat: var(--color-none-repeat);
			background-position: var(--color-none-position);
			background-size: var(--color-none-size-24px);
			background-image: var(--color-none-image-24px);
		}

		.chip {
			position: absolute;
			bottom: -1px;
			right: 0;
			height: 13px;
			line-height: 13px;
			background: var(--color-f-white);
			color: var(--color-2-mildblack);
			border-radius: 4px 0 0 0;
			padding: 0 4px;
			font-size: 10px;
			box-shadow: 0 0 2px var(--color-3-darkgray);
		}
	}

	&.color-input.color-input > button {
		outline-offset: 0;
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
import TextLabel from "@/components/widgets/labels/TextLabel.vue";

export default defineComponent({
	emits: ["update:value", "update:open"],
	props: {
		value: { type: Color as PropType<Color>, required: true },
		noTransparency: { type: Boolean as PropType<boolean>, default: false }, // TODO: Rename to allowTransparency, also implement allowNone
		disabled: { type: Boolean as PropType<boolean>, default: false }, // TODO: Design and implement
		tooltip: { type: String as PropType<string | undefined>, required: false },
		sharpRightCorners: { type: Boolean as PropType<boolean>, default: false },

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
	computed: {
		chip() {
			return undefined;
		},
	},
	components: {
		ColorPicker,
		LayoutRow,
		TextLabel,
	},
});
</script>

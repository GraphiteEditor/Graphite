<script lang="ts">
import { defineComponent, type PropType } from "vue";

import { type Color } from "@/wasm-communication/messages";

import ColorPicker from "@/components/floating-menus/ColorPicker.vue";
import LayoutCol from "@/components/layout/LayoutCol.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";

export default defineComponent({
	inject: ["editor"],
	props: {
		primary: { type: Object as PropType<Color>, required: true },
		secondary: { type: Object as PropType<Color>, required: true },
	},
	data() {
		return {
			primaryOpen: false,
			secondaryOpen: false,
		};
	},
	methods: {
		clickPrimarySwatch() {
			this.primaryOpen = true;
			this.secondaryOpen = false;
		},
		clickSecondarySwatch() {
			this.primaryOpen = false;
			this.secondaryOpen = true;
		},
		primaryColorChanged(color: Color) {
			this.editor.instance.updatePrimaryColor(color.red, color.green, color.blue, color.alpha);
		},
		secondaryColorChanged(color: Color) {
			this.editor.instance.updateSecondaryColor(color.red, color.green, color.blue, color.alpha);
		},
	},
	components: {
		ColorPicker,
		LayoutCol,
		LayoutRow,
	},
});
</script>

<template>
	<LayoutCol class="swatch-pair">
		<LayoutRow class="primary swatch">
			<button @click="() => clickPrimarySwatch()" :style="{ '--swatch-color': primary.toRgbaCSS() }" data-floating-menu-spawner="no-hover-transfer" tabindex="0"></button>
			<ColorPicker v-model:open="primaryOpen" :color="primary" @update:color="(color: Color) => primaryColorChanged(color)" :direction="'Right'" />
		</LayoutRow>
		<LayoutRow class="secondary swatch">
			<button @click="() => clickSecondarySwatch()" :style="{ '--swatch-color': secondary.toRgbaCSS() }" data-floating-menu-spawner="no-hover-transfer" tabindex="0"></button>
			<ColorPicker v-model:open="secondaryOpen" :color="secondary" @update:color="(color: Color) => secondaryColorChanged(color)" :direction="'Right'" />
		</LayoutRow>
	</LayoutCol>
</template>

<style lang="scss">
.swatch-pair {
	flex: 0 0 auto;

	.swatch {
		width: 28px;
		height: 28px;
		margin: 0 2px;
		position: relative;

		> button {
			--swatch-color: #ffffff;
			width: 100%;
			height: 100%;
			border-radius: 50%;
			border: 2px var(--color-5-dullgray) solid;
			box-shadow: 0 0 0 2px var(--color-3-darkgray);
			margin: 0;
			padding: 0;
			box-sizing: border-box;
			background: linear-gradient(var(--swatch-color), var(--swatch-color)), var(--color-transparent-checkered-background);
			background-size: var(--color-transparent-checkered-background-size);
			background-position: var(--color-transparent-checkered-background-position);
			overflow: hidden;
		}

		.floating-menu {
			top: 50%;
			right: -2px;
		}

		&.primary {
			margin-bottom: -8px;
			z-index: 1;
		}
	}
}
</style>

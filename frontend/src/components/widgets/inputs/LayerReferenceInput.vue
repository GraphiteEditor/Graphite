<template>
	<LayoutRow
		class="layer-reference-input"
		:class="{ disabled, droppable, 'sharp-right-corners': sharpRightCorners }"
		:title="tooltip"
		@dragover="(e: DragEvent) => !disabled && dragOver(e)"
		@dragleave="() => !disabled && dragLeave()"
		@drop="(e: DragEvent) => !disabled && drop(e)"
	>
		<template v-if="value === undefined || droppable">
			<LayoutRow class="drop-zone"></LayoutRow>
			<TextLabel :italic="true">{{ droppable ? "Drop" : "Drag" }} Layer Here</TextLabel>
		</template>
		<template v-if="value !== undefined && !droppable">
			<IconLabel v-if="layerName !== undefined && layerType" :icon="layerTypeData(layerType).icon" class="layer-icon" />
			<TextLabel v-if="layerName !== undefined && layerType" :italic="layerName === ''" class="layer-name">{{ layerName || layerTypeData(layerType).name }}</TextLabel>
			<TextLabel :bold="true" :italic="true" v-else class="missing">Layer Missing</TextLabel>
		</template>
		<IconButton v-if="value !== undefined && !droppable" :icon="'CloseX'" :size="16" :disabled="disabled" :action="() => clearLayer()" />
	</LayoutRow>
</template>

<style lang="scss">
.layer-reference-input {
	position: relative;
	flex: 1 0 auto;
	height: 24px;
	border-radius: 2px;
	background: var(--color-1-nearblack);

	.drop-zone {
		pointer-events: none;
		border: 1px dashed var(--color-5-dullgray);
		border-radius: 1px;
		position: absolute;
		top: 2px;
		bottom: 2px;
		left: 2px;
		right: 2px;
	}

	&.droppable .drop-zone {
		border: 1px dashed var(--color-e-nearwhite);
	}

	.layer-icon {
		margin: 4px 8px;

		+ .text-label {
			padding-left: 0;
		}
	}

	.text-label {
		line-height: 18px;
		padding: 3px calc(8px + 2px);
		width: 100%;
		text-align: center;

		&.missing {
			color: var(--color-data-unused1);
		}

		&.layer-name {
			text-align: left;
		}
	}

	.icon-button {
		margin: 4px;
		margin-left: 0;
	}

	&.disabled {
		background: var(--color-2-mildblack);

		.drop-zone {
			border: 1px dashed var(--color-4-dimgray);
		}

		.text-label {
			color: var(--color-8-uppergray);
		}

		.icon-label svg {
			fill: var(--color-8-uppergray);
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, type PropType } from "vue";

import { currentDraggingElement } from "@/io-managers/drag";

import type { LayerType, LayerTypeData } from "@/wasm-communication/messages";
import { layerTypeData } from "@/wasm-communication/messages";

import LayoutRow from "@/components/layout/LayoutRow.vue";
import IconButton from "@/components/widgets/buttons/IconButton.vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";
import TextLabel from "@/components/widgets/labels/TextLabel.vue";

export default defineComponent({
	emits: ["update:value"],
	props: {
		value: { type: String as PropType<string | undefined>, required: false },
		layerName: { type: String as PropType<string | undefined>, required: false },
		layerType: { type: String as PropType<LayerType | undefined>, required: false },
		disabled: { type: Boolean as PropType<boolean>, default: false },
		tooltip: { type: String as PropType<string | undefined>, required: false },
		sharpRightCorners: { type: Boolean as PropType<boolean>, default: false },
	},
	data() {
		return {
			hoveringDrop: false,
		};
	},
	computed: {
		droppable() {
			return this.hoveringDrop && currentDraggingElement();
		},
	},
	methods: {
		dragOver(e: DragEvent): void {
			this.hoveringDrop = true;

			e.preventDefault();
		},
		dragLeave(): void {
			this.hoveringDrop = false;
		},
		drop(e: DragEvent): void {
			this.hoveringDrop = false;

			const element = currentDraggingElement();
			const layerPath = element?.getAttribute("data-layer") || undefined;

			if (layerPath) {
				e.preventDefault();

				this.$emit("update:value", layerPath);
			}
		},
		clearLayer(): void {
			this.$emit("update:value", undefined);
		},
		layerTypeData(layerType: LayerType): LayerTypeData {
			return layerTypeData(layerType) || { name: "Error", icon: "Info" };
		},
	},
	components: {
		IconButton,
		IconLabel,
		LayoutRow,
		TextLabel,
	},
});
</script>

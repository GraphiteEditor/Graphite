<template>
	<LayoutRow
		class="layer-reference-input"
		:class="{ disabled, droppable, 'sharp-right-corners': sharpRightCorners }"
		:title="tooltip"
		@dragover="(e: DragEvent) => !disabled && dragOver(e)"
		@dragleave="() => !disabled && dragLeave()"
		@drop="(e: DragEvent) => !disabled && drop(e)"
	>
		<LayoutRow v-if="value === undefined || droppable" class="drop-zone"></LayoutRow>
		<TextLabel v-if="value === undefined || droppable" :italic="true">Drag Layer Here</TextLabel>
		<TextLabel v-if="value !== undefined && !droppable">{{ display || "Layer Missing" }}</TextLabel>
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

	.text-label {
		line-height: 18px;
		padding: 3px calc(8px + 2px);
		width: 100%;
		text-align: center;
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

import LayoutRow from "@/components/layout/LayoutRow.vue";
import IconButton from "@/components/widgets/buttons/IconButton.vue";
import TextLabel from "@/components/widgets/labels/TextLabel.vue";

export default defineComponent({
	emits: ["update:value"],
	props: {
		value: { type: String as PropType<string | undefined>, required: false },
		display: { type: String as PropType<string | undefined>, required: false },
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
	},
	components: {
		IconButton,
		LayoutRow,
		TextLabel,
	},
});
</script>

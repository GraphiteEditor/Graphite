<template>
	<FloatingMenu
		:open="true"
		class="eyedropper-preview"
		:type="'Cursor'"
		:style="{ '--ring-color-primary': primaryColor, '--ring-color-secondary': secondaryColor, '--ring-color-choice': colorChoice }"
	>
		<div class="ring">
			<div class="zoomed-preview-container" :class="samplingPrimaryOrSecondary">
				<canvas class="zoomed-preview" ref="zoomPreviewCanvas"></canvas>
				<div class="pixel-outline"></div>
			</div>
		</div>
	</FloatingMenu>
</template>

<style lang="scss">
.eyedropper-preview {
	pointer-events: none;

	.ring {
		transform: translate(0, -50%) rotate(45deg);
		position: relative;
		background: var(--ring-color-choice);
		padding: 16px;
		border: 8px solid;
		border-radius: 50%;
		border-top-color: var(--ring-color-primary);
		border-left-color: var(--ring-color-primary);
		border-bottom-color: var(--ring-color-secondary);
		border-right-color: var(--ring-color-secondary);

		.zoomed-preview-container {
			transform: rotate(-45deg);

			&.primary::before,
			&.secondary::before {
				content: "";
				width: 100%;
				height: 100%;
				padding: 16px;
				margin: calc(-16px - 8px - 8px);
				transform: rotate(45deg);
				position: absolute;
				border: 16px solid;
				border-radius: 50%;
			}

			&.primary::before {
				border-top-color: var(--ring-color-primary);
				border-left-color: var(--ring-color-primary);
				border-bottom-color: transparent;
				border-right-color: transparent;
			}

			&.secondary::before {
				border-top-color: transparent;
				border-left-color: transparent;
				border-bottom-color: var(--ring-color-secondary);
				border-right-color: var(--ring-color-secondary);
			}

			.zoomed-preview {
				border-radius: 50%;
				width: 110px;
				height: 110px;
				image-rendering: pixelated;
			}

			.pixel-outline {
				position: absolute;
				left: 50%;
				top: 50%;
				transform: translate(-50%, -50%);
				--outline-width: 2;
				margin-top: calc(-1px * (var(--outline-width) / 2));
				width: calc(10px - (var(--outline-width) * 1px));
				height: calc(10px - var(--outline-width) * 1px);
				border: calc(var(--outline-width) * 1px) solid black;
			}
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, type PropType } from "vue";

import FloatingMenu from "@/components/floating-menus/FloatingMenu.vue";

// Should be equal to the width and height of the canvas in the CSS above
const ZOOM_WINDOW_DIMENSIONS_EXPANDED = 110;
// SHould be equal to the width and height of the `.pixel-outline` div in the CSS above, and should be evenly divisible into the number above
const UPSCALE_FACTOR = 10;

export const ZOOM_WINDOW_DIMENSIONS = ZOOM_WINDOW_DIMENSIONS_EXPANDED / UPSCALE_FACTOR;

const temporaryCanvas = document.createElement("canvas");

export default defineComponent({
	props: {
		imageData: { type: Object as PropType<ImageData> },
		colorChoice: { type: String as PropType<string>, required: true },
		primaryColor: { type: String as PropType<string>, required: true },
		secondaryColor: { type: String as PropType<string>, required: true },
		samplingPrimaryOrSecondary: { type: String as PropType<"primary" | "secondary" | "">, default: "" },
	},
	mounted() {
		this.displayImageDataPreview(this.imageData);
	},
	watch: {
		imageData(imageData: ImageData | undefined) {
			this.displayImageDataPreview(imageData);
		},
	},
	methods: {
		displayImageDataPreview(imageData: ImageData | undefined) {
			const canvas = this.$refs.zoomPreviewCanvas as HTMLCanvasElement;
			canvas.width = ZOOM_WINDOW_DIMENSIONS;
			canvas.height = ZOOM_WINDOW_DIMENSIONS;
			const context = canvas.getContext("2d");

			temporaryCanvas.width = ZOOM_WINDOW_DIMENSIONS;
			temporaryCanvas.height = ZOOM_WINDOW_DIMENSIONS;
			const temporaryContext = temporaryCanvas.getContext("2d");

			if (!imageData || !context || !temporaryContext) return;

			temporaryContext.putImageData(imageData, 0, 0, 0, 0, ZOOM_WINDOW_DIMENSIONS, ZOOM_WINDOW_DIMENSIONS);

			context.fillStyle = "black";
			context.fillRect(0, 0, ZOOM_WINDOW_DIMENSIONS, ZOOM_WINDOW_DIMENSIONS);

			context.drawImage(temporaryCanvas, 0, 0);
		},
	},
	components: {
		FloatingMenu,
	},
});
</script>

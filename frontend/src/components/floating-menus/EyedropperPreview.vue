<template>
	<FloatingMenu
		:open="true"
		class="eyedropper-preview"
		:type="'Cursor'"
		:style="{ '--ring-color-primary': primaryColor, '--ring-color-secondary': secondaryColor, '--ring-color-choice': colorChoice }"
	>
		<div class="ring">
			<div class="canvas-container">
				<canvas ref="zoomPreviewCanvas"></canvas>
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

		&::after {
			content: "";
			position: absolute;
			width: 100%;
			height: 100%;
			top: -9px;
			left: -9px;
			padding: 8px;
			border-radius: 50%;
			border: 1px solid rgba(255, 255, 255, 0.25);
			box-shadow: 0 0 8px rgba(0, 0, 0, 0.25);
		}

		.canvas-container {
			transform: rotate(-45deg);

			canvas {
				display: block;
				width: 110px;
				height: 110px;
				border-radius: 50%;
				image-rendering: pixelated;
				border: 1px solid rgba(255, 255, 255, 0.25);
			}

			&::after {
				content: "";
				position: absolute;
				top: 1px;
				left: 1px;
				width: calc(100% - 2px);
				height: calc(100% - 2px);
				border-radius: 50%;
				box-shadow: inset 0 0 8px rgba(0, 0, 0, 0.25);
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
				border: calc(var(--outline-width) * 1px) solid var(--color-0-black);
			}
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, type PropType } from "vue";

import FloatingMenu from "@/components/layout/FloatingMenu.vue";

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

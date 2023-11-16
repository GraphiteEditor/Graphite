<script lang="ts" context="module">
	// Should be equal to the width and height of the zoom preview canvas in the CSS
	const ZOOM_WINDOW_DIMENSIONS_EXPANDED = 110;
	// Should be equal to the width and height of the `.pixel-outline` div in the CSS, and should be evenly divisible into the number above
	const UPSCALE_FACTOR = 10;

	export const ZOOM_WINDOW_DIMENSIONS = ZOOM_WINDOW_DIMENSIONS_EXPANDED / UPSCALE_FACTOR;
</script>

<script lang="ts">
	import { onMount } from "svelte";

	import FloatingMenu from "@graphite/components/layout/FloatingMenu.svelte";

	const temporaryCanvas = document.createElement("canvas");
	temporaryCanvas.width = ZOOM_WINDOW_DIMENSIONS;
	temporaryCanvas.height = ZOOM_WINDOW_DIMENSIONS;

	let zoomPreviewCanvas: HTMLCanvasElement | undefined;

	export let imageData: ImageData | undefined = undefined;
	export let colorChoice: string;
	export let primaryColor: string;
	export let secondaryColor: string;
	export let x: number;
	export let y: number;

	$: displayImageDataPreview(imageData);

	function displayImageDataPreview(imageData: ImageData | undefined) {
		if (!zoomPreviewCanvas) return;
		const context = zoomPreviewCanvas.getContext("2d");

		const temporaryContext = temporaryCanvas.getContext("2d");

		if (!imageData || !context || !temporaryContext) return;

		temporaryContext.putImageData(imageData, 0, 0, 0, 0, ZOOM_WINDOW_DIMENSIONS, ZOOM_WINDOW_DIMENSIONS);

		context.fillStyle = "black";
		context.fillRect(0, 0, ZOOM_WINDOW_DIMENSIONS, ZOOM_WINDOW_DIMENSIONS);

		context.drawImage(temporaryCanvas, 0, 0);
	}

	onMount(() => {
		displayImageDataPreview(imageData);
	});
</script>

<FloatingMenu
	open={true}
	class="eyedropper-preview"
	type="Cursor"
	styles={{ "--ring-color-primary": primaryColor, "--ring-color-secondary": secondaryColor, "--ring-color-choice": colorChoice, left: x + "px", top: y + "px" }}
>
	<div class="ring">
		<div class="canvas-container">
			<canvas width={ZOOM_WINDOW_DIMENSIONS} height={ZOOM_WINDOW_DIMENSIONS} bind:this={zoomPreviewCanvas} />
			<div class="pixel-outline" />
		</div>
	</div>
</FloatingMenu>

<style lang="scss" global>
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
				top: -8px;
				left: -8px;
				padding: 8px;
				border-radius: 50%;
				box-shadow:
					0 0 0 1px rgba(255, 255, 255, 0.5),
					0 0 8px rgba(0, 0, 0, 0.25);
			}

			.canvas-container {
				transform: rotate(-45deg);

				canvas {
					display: block;
					width: 110px;
					height: 110px;
					border-radius: 50%;
					image-rendering: pixelated;
				}

				&::after {
					content: "";
					position: absolute;
					top: 0;
					left: 0;
					width: 100%;
					height: 100%;
					border-radius: 50%;
					box-shadow:
						inset 0 0 0 1px rgba(255, 255, 255, 0.5),
						inset 0 0 8px rgba(0, 0, 0, 0.25);
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

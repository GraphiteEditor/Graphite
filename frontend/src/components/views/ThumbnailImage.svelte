<script lang="ts">
	import { afterUpdate } from "svelte";

	export let image: ImageData | undefined;

	let canvas: HTMLCanvasElement | undefined;

	// After the update, since resizing the canvas clears it
	afterUpdate(() => {
		if (image) canvas?.getContext("2d")?.putImageData(image, 0, 0);
	});
</script>

<canvas bind:this={canvas} width={image?.width} height={image?.height}></canvas>

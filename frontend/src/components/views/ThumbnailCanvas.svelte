<script lang="ts">
	import { afterUpdate } from "svelte";

	export let canvasId: number | undefined;

	let container: HTMLDivElement | undefined;

	// The graph runtime draws the thumbnail into this canvas, which is moved here rather than copied
	afterUpdate(() => {
		const canvas = canvasId !== undefined ? window.imageCanvases[String(canvasId)] : undefined;
		if (!container || !canvas || canvas.parentElement === container) return;

		// eslint-disable-next-line svelte/no-dom-manipulating
		container.replaceChildren(canvas);
	});
</script>

<div bind:this={container} style:display="contents"></div>

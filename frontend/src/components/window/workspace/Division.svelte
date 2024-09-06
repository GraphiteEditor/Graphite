<script lang="ts">
	import { getContext } from "svelte";

	import { MIN_PANEL_SIZE } from "@graphite/state-providers/dockspace";
	import type { Division } from "@graphite/wasm-communication/messages";

	import type Editor from "@graphite/components/Editor.svelte";
	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import SubdivisionOrPanel from "@graphite/components/window/workspace/SubdivisionOrPanel.svelte";

	export let divisionData: Division;

	$: isHorizontal = divisionData.direction === "Horizontal";
	$: directionComponent = isHorizontal ? LayoutRow : LayoutCol;

	$: startFlexBasis = divisionData.startSize;
	$: endFlexBasis = divisionData.endSize;

	const editor = getContext<Editor>("editor");

	function resizePanel(e: PointerEvent) {
		const gutter = e.target;
		if (!(gutter instanceof HTMLElement)) return;
		const startElement = gutter?.previousElementSibling;
		const endElement = gutter?.nextElementSibling;
		if (!(startElement instanceof HTMLElement) || !(endElement instanceof HTMLElement)) return;

		// Get the current size in px of the panels being resized and the gutter
		const startSize = isHorizontal ? startElement.getBoundingClientRect().width : startElement.getBoundingClientRect().height;
		const endSize = isHorizontal ? endElement.getBoundingClientRect().width : endElement.getBoundingClientRect().height;

		// Prevent cursor flicker as mouse temporarily leaves the gutter
		gutter.setPointerCapture(e.pointerId);

		const mouseStart = isHorizontal ? e.clientX : e.clientY;

		const updatePosition = (e: PointerEvent) => {
			const mouseCurrent = isHorizontal ? e.clientX : e.clientY;
			let mouseDelta = mouseCurrent - mouseStart;

			mouseDelta = endSize - Math.max(endSize - mouseDelta, MIN_PANEL_SIZE);
			mouseDelta = Math.max(startSize + mouseDelta, MIN_PANEL_SIZE) - startSize;

			startFlexBasis = startSize + mouseDelta;
			endFlexBasis = endSize - mouseDelta;
		};

		const cleanup = (e: PointerEvent) => {
			editor.handle.resizeDivision(divisionData.identifier, startFlexBasis, endFlexBasis);
			gutter.releasePointerCapture(e.pointerId);

			document.removeEventListener("pointermove", updatePosition);
			document.removeEventListener("pointerup", cleanup);
		};

		document.addEventListener("pointermove", updatePosition);
		document.addEventListener("pointerup", cleanup);
	}
</script>

<svelte:component this={directionComponent}>
	<LayoutCol class="workspace-grid-subdivision" styles={{ "flex-grow": startFlexBasis }}>
		<SubdivisionOrPanel value={divisionData.start} />
	</LayoutCol>
	<LayoutCol class={`workspace-grid-resize-gutter ${divisionData.direction.toLowerCase()}`} on:pointerdown={resizePanel} />
	<LayoutCol class="workspace-grid-subdivision" styles={{ "flex-grow": endFlexBasis }}>
		<SubdivisionOrPanel value={divisionData.end} />
	</LayoutCol>
</svelte:component>

<style lang="scss" global>
	.workspace-grid-subdivision {
		min-height: 28px;
		flex: 1 1 0;

		&.folded {
			flex-grow: 0;
			height: 0;
		}
	}

	.workspace-grid-resize-gutter {
		flex: 0 0 4px;

		&.vertical {
			cursor: ns-resize;
		}

		&.horizontal {
			cursor: ew-resize;
		}
	}
</style>

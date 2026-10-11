<script lang="ts">
	import { getContext } from "svelte";
	import type { NodeGraphStore } from "/src/stores/node-graph";

	const nodeGraph = getContext<NodeGraphStore>("nodeGraph");
	const nodeGraphTransform = nodeGraph.transformStore;

	// The wire is drawn above every panel so it can reach connectors in the Properties panel, which means mapping graph space onto the window ourselves
	$: wirePath = $nodeGraph.wirePathInProgress;
	$: graphBounds = wirePath && document.querySelector("[data-node-graph]")?.getBoundingClientRect();
	$: offsetX = (graphBounds?.left || 0) + $nodeGraphTransform.x;
	$: offsetY = (graphBounds?.top || 0) + $nodeGraphTransform.y;
</script>

{#if wirePath && graphBounds}
	<svg class="wire-in-progress">
		<path
			d={wirePath.pathString}
			transform={`translate(${offsetX} ${offsetY}) scale(${$nodeGraphTransform.scale})`}
			style:--data-line-width={`${wirePath.thick ? 8 : 2}px`}
			style:--data-color={`var(--color-data-${wirePath.dataType.toLowerCase()})`}
			style:--data-color-dim={`var(--color-data-${wirePath.dataType.toLowerCase()}-dim)`}
			style:--data-dasharray={`3,${wirePath.dashed ? 2 : 0}`}
		/>
	</svg>
{/if}

<style lang="scss">
	.wire-in-progress {
		position: fixed;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		pointer-events: none;
		overflow: visible;
		z-index: 1000;

		path {
			fill: none;
			stroke: var(--data-color-dim);
			stroke-width: var(--data-line-width);
			stroke-dasharray: var(--data-dasharray);
		}
	}
</style>

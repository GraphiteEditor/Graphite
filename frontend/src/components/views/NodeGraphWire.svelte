<script lang="ts">
	import type { Readable } from "svelte/store";
	import type { WirePath } from "/wrapper/pkg/graphite_wasm_wrapper";

	export let wire: Readable<WirePath>;
	export let thick: boolean;
	export let undimmed: boolean;

	$: dataColor = `var(--color-data-${$wire.dataType.toLowerCase()})`;
	$: dataColorDim = `var(--color-data-${$wire.dataType.toLowerCase()}-dim)`;
	$: dasharray = `3,${$wire.dashed ? 2 : 0}`;
</script>

{#if $wire.thick === thick}
	<g class:dashed={$wire.dashed} class:undimmed>
		{#if thick}
			<path d={$wire.pathString} style:--data-line-width="8px" style:--data-color={dataColor} style:--data-color-dim={dataColorDim} style:--data-dasharray={dasharray} />
			<!-- A thin inner line splits the layer-stack wire down the middle reaching past the ends into the cleaved connector slots -->
			<path d={$wire.centerPathString} style:--data-line-width="2px" style:--data-color="#444444" style:--data-color-dim="#444444" style:--data-dasharray={dasharray} />
		{:else if $wire.isList}
			<!-- A rank-1 List wire reads as two parallel lines: a triple-width data line split down the middle by a 1x background-colored overlay -->
			<path d={$wire.pathString} style:--data-line-width="4px" style:--data-color={dataColor} style:--data-color-dim={dataColorDim} style:--data-dasharray={dasharray} />
			<path d={$wire.pathString} style:--data-line-width="2px" style:--data-color="#444444" style:--data-color-dim="#444444" style:--data-dasharray={dasharray} />
		{:else}
			<path d={$wire.pathString} style:--data-line-width="2px" style:--data-color={dataColor} style:--data-color-dim={dataColorDim} style:--data-dasharray={dasharray} />
		{/if}
	</g>
{/if}

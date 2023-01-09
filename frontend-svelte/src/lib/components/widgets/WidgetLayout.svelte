<script lang="ts">
	import { isWidgetColumn, isWidgetRow, isWidgetSection, type WidgetLayout } from "$lib/wasm-communication/messages";

	import WidgetSection from "$lib/components/widgets/groups/WidgetSection.svelte";
	import WidgetRow from "$lib/components/widgets/WidgetRow.svelte";

	export let layout: WidgetLayout;
</script>

<!-- TODO: Refactor this component (together with `WidgetRow.vue`) to be more logically consistent with our layout definition goals, in terms of naming and capabilities -->

<template>
	<div class="widget-layout">
		{#each layout.layout as layoutGroup, index (index)}
			{#if isWidgetColumn(layoutGroup) || isWidgetRow(layoutGroup)}
				<WidgetRow widgetData={layoutGroup} layoutTarget={layout.layoutTarget} />
			{:else if isWidgetSection(layoutGroup)}
				<WidgetSection widgetData={layoutGroup} layoutTarget={layout.layoutTarget} />
			{:else}
				<span style="color: red">Error: The widget row that belongs here has an invalid layout group type</span>
			{/if}
		{/each}
	</div>
</template>

<style lang="scss" global>
	.widget-layout {
		height: 100%;
		flex: 0 0 auto;
		display: flex;
		flex-direction: column;
	}
</style>

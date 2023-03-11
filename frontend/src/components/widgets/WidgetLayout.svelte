<script lang="ts">
	import { isWidgetColumn, isWidgetRow, isWidgetSection, type WidgetLayout } from "~/src/wasm-communication/messages";

	import WidgetSection from "~/src/components/widgets/groups/WidgetSection.svelte";
	import WidgetRow from "~/src/components/widgets/WidgetRow.svelte";

	export let layout: WidgetLayout;
	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};

	$: extraClasses = Object.entries(classes)
		.flatMap((classAndState) => (classAndState[1] ? [classAndState[0]] : []))
		.join(" ");
</script>

<!-- TODO: Refactor this component (together with `WidgetRow.svelte`) to be more logically consistent with our layout definition goals, in terms of naming and capabilities -->
<div class={`widget-layout ${className} ${extraClasses}`.trim()}>
	{#each layout.layout as layoutGroup, index (index)}
		{#if isWidgetColumn(layoutGroup) || isWidgetRow(layoutGroup)}
			<WidgetRow widgetData={layoutGroup} layoutTarget={layout.layoutTarget} />
		{:else if isWidgetSection(layoutGroup)}
			<WidgetSection widgetData={layoutGroup} layoutTarget={layout.layoutTarget} />
		{:else}
			<span style="color: #d6536e">Error: The widget row that belongs here has an invalid layout group type</span>
		{/if}
	{/each}
</div>

<style lang="scss" global>
	.widget-layout {
		height: 100%;
		flex: 0 0 auto;
		display: flex;
		flex-direction: column;
	}
</style>

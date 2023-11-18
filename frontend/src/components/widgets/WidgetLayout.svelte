<script lang="ts">
	import { isWidgetSpanColumn, isWidgetSpanRow, isWidgetSection, type WidgetLayout } from "@graphite/wasm-communication/messages";

	import WidgetSection from "@graphite/components/widgets/WidgetSection.svelte";
	import WidgetSpan from "@graphite/components/widgets/WidgetSpan.svelte";

	export let layout: WidgetLayout;
	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};

	$: extraClasses = Object.entries(classes)
		.flatMap((classAndState) => (classAndState[1] ? [classAndState[0]] : []))
		.join(" ");
</script>

<div class={`widget-layout ${className} ${extraClasses}`.trim()}>
	{#each layout.layout as layoutGroup}
		{#if isWidgetSpanRow(layoutGroup) || isWidgetSpanColumn(layoutGroup)}
			<WidgetSpan widgetData={layoutGroup} layoutTarget={layout.layoutTarget} />
		{:else if isWidgetSection(layoutGroup)}
			<WidgetSection widgetData={layoutGroup} layoutTarget={layout.layoutTarget} />
		{:else}
			<span style="color: #d6536e">Error: The widget layout that belongs here has an invalid layout group type</span>
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

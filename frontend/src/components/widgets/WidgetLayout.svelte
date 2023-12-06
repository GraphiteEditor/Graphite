<script lang="ts">
	import { isWidgetSpanColumn, isWidgetSpanRow, isWidgetSection, type WidgetLayout } from "@graphite/wasm-communication/messages";

	import WidgetSection from "@graphite/components/widgets/WidgetSection.svelte";
	import WidgetSpan from "@graphite/components/widgets/WidgetSpan.svelte";

	export let layout: WidgetLayout;
	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};
</script>

{#each layout.layout as layoutGroup}
	{#if isWidgetSpanRow(layoutGroup) || isWidgetSpanColumn(layoutGroup)}
		<WidgetSpan widgetData={layoutGroup} layoutTarget={layout.layoutTarget} class={className} {classes} />
	{:else if isWidgetSection(layoutGroup)}
		<WidgetSection widgetData={layoutGroup} layoutTarget={layout.layoutTarget} class={className} {classes} />
	{:else}
		<span style="color: #d6536e">Error: The widget layout that belongs here has an invalid layout group type</span>
	{/if}
{/each}

<style lang="scss" global></style>

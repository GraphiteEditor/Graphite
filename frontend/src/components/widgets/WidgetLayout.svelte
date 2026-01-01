<script lang="ts">
	import { isWidgetSpanColumn, isWidgetSpanRow, isWidgetSection, type Layout, isWidgetTable, type LayoutTarget } from "@graphite/messages";

	import WidgetSection from "@graphite/components/widgets/WidgetSection.svelte";
	import WidgetSpan from "@graphite/components/widgets/WidgetSpan.svelte";
	import WidgetTable from "@graphite/components/widgets/WidgetTable.svelte";

	export let layout: Layout;
	export let layoutTarget: LayoutTarget;
	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};
</script>

{#each layout as layoutGroup}
	{#if isWidgetSpanRow(layoutGroup) || isWidgetSpanColumn(layoutGroup)}
		<WidgetSpan widgetData={layoutGroup} {layoutTarget} class={className} {classes} />
	{:else if isWidgetSection(layoutGroup)}
		<WidgetSection widgetData={layoutGroup} {layoutTarget} class={className} {classes} />
	{:else if isWidgetTable(layoutGroup)}
		<WidgetTable widgetData={layoutGroup} {layoutTarget} unstyled={layoutGroup.unstyled} />
	{/if}
{/each}

<style lang="scss" global></style>

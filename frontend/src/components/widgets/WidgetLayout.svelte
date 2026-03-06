<script lang="ts">
	import type { Layout, LayoutTarget } from "@graphite/messages";
	import { isWidgetSpanColumn, isWidgetSpanRow, isWidgetTable, isWidgetSection } from "@graphite/utility-functions/widgets";

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
		<WidgetTable widgetData={layoutGroup} {layoutTarget} unstyled={layoutGroup.table.unstyled} />
	{/if}
{/each}

<style lang="scss" global></style>

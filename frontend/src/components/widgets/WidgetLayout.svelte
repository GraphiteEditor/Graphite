<script lang="ts">
	import type { Layout, LayoutTarget } from "@graphite/../wasm/pkg/graphite_wasm";

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
	{#if "row" in layoutGroup}
		<WidgetSpan direction="row" widgets={layoutGroup.row.rowWidgets} {layoutTarget} class={className} {classes} />
	{:else if "column" in layoutGroup}
		<WidgetSpan direction="column" widgets={layoutGroup.column.columnWidgets} {layoutTarget} class={className} {classes} />
	{:else if "section" in layoutGroup}
		<WidgetSection widgetData={layoutGroup.section} {layoutTarget} class={className} {classes} />
	{:else if "table" in layoutGroup}
		<WidgetTable widgetData={layoutGroup.table} {layoutTarget} />
	{/if}
{/each}

<style lang="scss" global></style>

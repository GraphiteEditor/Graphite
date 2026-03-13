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
	{#if "Row" in layoutGroup}
		<WidgetSpan direction="row" widgets={layoutGroup.Row.rowWidgets} {layoutTarget} class={className} {classes} />
	{:else if "Column" in layoutGroup}
		<WidgetSpan direction="column" widgets={layoutGroup.Column.columnWidgets} {layoutTarget} class={className} {classes} />
	{:else if "Section" in layoutGroup}
		<WidgetSection widgetData={layoutGroup.Section} {layoutTarget} class={className} {classes} />
	{:else if "Table" in layoutGroup}
		<WidgetTable widgetData={layoutGroup.Table} {layoutTarget} />
	{/if}
{/each}

<style lang="scss" global></style>

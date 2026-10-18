<script lang="ts">
	import NodeSection from "/src/components/widgets/NodeSection.svelte";
	import WidgetSpan from "/src/components/widgets/WidgetSpan.svelte";
	import WidgetTable from "/src/components/widgets/WidgetTable.svelte";
	import type { Layout, LayoutTarget } from "/wrapper/pkg/graphite_wasm_wrapper";

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
	{:else if "NodeSection" in layoutGroup}
		<NodeSection widgetData={layoutGroup.NodeSection} {layoutTarget} class={className} {classes} />
	{:else if "Table" in layoutGroup}
		<WidgetTable widgetData={layoutGroup.Table} {layoutTarget} />
	{/if}
{/each}

<style lang="scss"></style>

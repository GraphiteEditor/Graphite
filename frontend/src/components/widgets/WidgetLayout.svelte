<script lang="ts">
	import NodeSection from "/src/components/widgets/NodeSection.svelte";
	import WidgetSpan from "/src/components/widgets/WidgetSpan.svelte";
	import WidgetTable from "/src/components/widgets/WidgetTable.svelte";
	import type { Layout, LayoutTarget } from "/wrapper/pkg/graphite_wasm_wrapper";

	type Props = {
		layout: Layout;
		layoutTarget: LayoutTarget;
		class?: string;
		classes?: Record<string, boolean>;
	};
	let { layout, layoutTarget, class: className = "", classes = {} }: Props = $props();
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

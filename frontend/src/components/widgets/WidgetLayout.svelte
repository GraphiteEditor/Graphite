<script lang="ts">
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";
	import WidgetSection from "@graphite/components/widgets/WidgetSection.svelte";
	import WidgetSpan from "@graphite/components/widgets/WidgetSpan.svelte";
	import WidgetTable from "@graphite/components/widgets/WidgetTable.svelte";
	import { isWidgetSpanColumn, isWidgetSpanRow, isWidgetSection, type WidgetLayout, isWidgetTable } from "@graphite/messages.svelte";

	type Props = {
		layout: WidgetLayout;
		class?: string;
		classes?: Record<string, boolean>;
	};

	let { layout, class: className = "", classes = {} }: Props = $props();
</script>

{#each layout.layout as layoutGroup}
	{#if isWidgetSpanRow(layoutGroup) || isWidgetSpanColumn(layoutGroup)}
		<WidgetSpan widgetData={layoutGroup} layoutTarget={layout.layoutTarget} class={className} {classes} />
	{:else if isWidgetSection(layoutGroup)}
		<WidgetSection widgetData={layoutGroup} layoutTarget={layout.layoutTarget} class={className} {classes} />
	{:else if isWidgetTable(layoutGroup)}
		<WidgetTable widgetData={layoutGroup} layoutTarget={layout.layoutTarget} />
	{:else}
		<TextLabel styles={{ color: "#d6536e" }}>Error: The widget layout that belongs here has an invalid layout group type</TextLabel>
	{/if}
{/each}

<style lang="scss" global></style>

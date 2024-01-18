<script lang="ts">
	import { isWidgetSpanRow, isWidgetSpanColumn, isWidgetSection, type WidgetSection as WidgetSectionFromJsMessages } from "@graphite/wasm-communication/messages";

	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";
	import WidgetSpan from "@graphite/components/widgets/WidgetSpan.svelte";

	export let widgetData: WidgetSectionFromJsMessages;
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	export let layoutTarget: any; // TODO: Give type

	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};

	let expanded = true;
</script>

<!-- TODO: Implement collapsable sections with properties system -->
<LayoutCol class={`widget-section ${className}`.trim()} {classes}>
	<button class="header" class:expanded on:click|stopPropagation={() => (expanded = !expanded)} tabindex="0">
		<div class="expand-arrow" />
		<TextLabel bold={true}>{widgetData.name}</TextLabel>
	</button>
	{#if expanded}
		<LayoutCol class="body">
			{#each widgetData.layout as layoutGroup}
				{#if isWidgetSpanRow(layoutGroup)}
					<WidgetSpan widgetData={layoutGroup} {layoutTarget} />
				{:else if isWidgetSpanColumn(layoutGroup)}
					<span style="color: #d6536e">Error: The WidgetSpan used here should be a row not a column</span>
				{:else if isWidgetSection(layoutGroup)}
					<svelte:self widgetData={layoutGroup} {layoutTarget} />
				{:else}
					<span style="color: #d6536e">Error: The widget that belongs here has an invalid layout group type</span>
				{/if}
			{/each}
		</LayoutCol>
	{/if}
</LayoutCol>

<style lang="scss" global>
	.widget-section {
		flex: 0 0 auto;
		margin: 0 4px;

		+ .widget-section {
			margin-top: 4px;
		}

		.header {
			text-align: left;
			align-items: center;
			display: flex;
			flex: 0 0 24px;
			padding: 0 8px;
			margin-bottom: 4px;
			border: 0;
			border-radius: 4px;
			background: var(--color-2-mildblack);

			.expand-arrow {
				width: 8px;
				height: 8px;
				margin: 0;
				padding: 0;
				position: relative;
				flex: 0 0 auto;
				display: flex;
				align-items: center;
				justify-content: center;

				&::after {
					content: "";
					position: absolute;
					width: 8px;
					height: 8px;
					background: var(--icon-expand-collapse-arrow);
				}
			}

			&.expanded {
				border-radius: 4px 4px 0 0;
				margin-bottom: 0;

				.expand-arrow::after {
					transform: rotate(90deg);
				}
			}

			.text-label {
				height: 18px;
				margin-left: 8px;
				display: inline-block;
			}

			&:hover {
				background: var(--color-4-dimgray);

				.expand-arrow::after {
					background: var(--icon-expand-collapse-arrow-hover);
				}

				+ .body {
					border: 1px solid var(--color-4-dimgray);
				}
			}
		}

		.body {
			padding: 0 7px;
			padding-top: 1px;
			margin-top: -1px;
			background: var(--color-3-darkgray);
			border: 1px solid var(--color-2-mildblack);
			border-radius: 0 0 4px 4px;
			overflow: hidden;

			.widget-span.row {
				&:first-child {
					margin-top: calc(4px - 1px);
				}

				&:last-child {
					margin-bottom: calc(4px - 1px);
				}

				> .text-button:first-child {
					margin-left: 16px;
				}

				> .text-label:first-of-type {
					flex: 0 0 25%;
					margin-left: 16px;
				}

				> .parameter-expose-button + .text-label:first-of-type {
					margin-left: 8px;
				}

				> .text-button {
					flex-grow: 1;
				}

				> .radio-input button {
					flex: 1 1 100%;
				}
			}
		}
	}
</style>

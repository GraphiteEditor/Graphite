<script lang="ts">
	import { isWidgetRow, isWidgetSection, type LayoutGroup, type WidgetSection as WidgetSectionFromJsMessages } from "@/wasm-communication/messages";

	import LayoutCol from "@/components/layout/LayoutCol.svelte";
	import TextLabel from "@/components/widgets/labels/TextLabel.svelte";
	import WidgetRow from "@/components/widgets/WidgetRow.svelte";
	import { getContext } from "svelte";
	import type { Editor } from "@/wasm-communication/editor";

	const editor = getContext<Editor>("editor");

	export let widgetData: WidgetSectionFromJsMessages;
	export let layoutTarget: any; // TODO: Give type

	let expanded = true;
</script>

<!-- TODO: Implement collapsable sections with properties system -->
<LayoutCol class="widget-section">
	<button class="header" class:expanded on:click|stopPropagation={() => (expanded = !expanded)} tabindex="0">
		<div class="expand-arrow" />
		<TextLabel bold={true}>{widgetData.name}</TextLabel>
	</button>
	{#if expanded}
		<LayoutCol class="body">
			{#each widgetData.layout as layoutGroup, index (index)}
				{#if isWidgetRow(layoutGroup)}
					<WidgetRow widgetData={layoutGroup} {layoutTarget} />
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

		.header {
			text-align: left;
			align-items: center;
			display: flex;
			flex: 0 0 24px;
			padding: 0 8px;
			margin-bottom: 4px;
			border: 0;
			border-radius: 4px;
			background: var(--color-5-dullgray);

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
				background: var(--color-6-lowergray);

				.expand-arrow::after {
					background: var(--icon-expand-collapse-arrow-hover);
				}

				.text-label {
					color: var(--color-f-white);
				}

				+ .body {
					border: 1px solid var(--color-6-lowergray);
				}
			}
		}

		.body {
			padding: 0 7px;
			padding-top: 1px;
			margin-top: -1px;
			margin-bottom: 4px;
			border: 1px solid var(--color-5-dullgray);
			border-radius: 0 0 4px 4px;
			overflow: hidden;

			.widget-row {
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

				> .parameter-expose-button ~ .text-label:first-of-type {
					margin-left: 0;
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

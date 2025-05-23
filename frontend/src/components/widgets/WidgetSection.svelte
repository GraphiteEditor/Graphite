<script lang="ts">
	import { getContext } from "svelte";

	import type { Editor } from "@graphite/editor";
	import { isWidgetSpanRow, isWidgetSpanColumn, isWidgetSection, type WidgetSection as WidgetSectionFromJsMessages } from "@graphite/messages";

	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import IconButton from "@graphite/components/widgets/buttons/IconButton.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";
	import WidgetSpan from "@graphite/components/widgets/WidgetSpan.svelte";

	export let widgetData: WidgetSectionFromJsMessages;
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	export let layoutTarget: any; // TODO: Give type

	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};

	let expanded = true;

	const editor = getContext<Editor>("editor");
</script>

<!-- TODO: Implement collapsable sections with properties system -->
<LayoutCol class={`widget-section ${className}`.trim()} {classes}>
	<button class="header" class:expanded on:click|stopPropagation={() => (expanded = !expanded)} tabindex="0">
		<div class="expand-arrow" />
		<TextLabel tooltip={widgetData.description} bold={true}>{widgetData.name}</TextLabel>
		<IconButton
			icon={widgetData.pinned ? "PinActive" : "PinInactive"}
			tooltip={widgetData.pinned ? "Unpin this node so it's no longer shown here when nothing is selected" : "Pin this node so it's shown here when nothing is selected"}
			size={24}
			action={(e) => {
				editor.handle.setNodePinned(widgetData.id, !widgetData.pinned);
				e?.stopPropagation();
			}}
			class={"show-only-on-hover"}
		/>
		<IconButton
			icon={"Trash"}
			tooltip={"Delete this node from the layer chain"}
			size={24}
			action={(e) => {
				editor.handle.deleteNode(widgetData.id);
				e?.stopPropagation();
			}}
			class={"show-only-on-hover"}
		/>
		<IconButton
			icon={widgetData.visible ? "EyeVisible" : "EyeHidden"}
			hoverIcon={widgetData.visible ? "EyeHide" : "EyeShow"}
			tooltip={widgetData.visible ? "Hide this node" : "Show this node"}
			size={24}
			action={(e) => {
				editor.handle.toggleNodeVisibilityLayerPanel(widgetData.id);
				e?.stopPropagation();
			}}
			class={widgetData.visible ? "show-only-on-hover" : ""}
		/>
	</button>
	{#if expanded}
		<LayoutCol class="body">
			{#each widgetData.layout as layoutGroup}
				{#if isWidgetSpanRow(layoutGroup)}
					<WidgetSpan widgetData={layoutGroup} {layoutTarget} />
				{:else if isWidgetSpanColumn(layoutGroup)}
					<TextLabel styles={{ color: "#d6536e" }}>Error: The WidgetSpan used here should be a row not a column</TextLabel>
				{:else if isWidgetSection(layoutGroup)}
					<svelte:self widgetData={layoutGroup} {layoutTarget} />
				{:else}
					<TextLabel styles={{ color: "#d6536e" }}>Error: The widget that belongs here has an invalid layout group type</TextLabel>
				{/if}
			{/each}
		</LayoutCol>
	{/if}
</LayoutCol>

<style lang="scss" global>
	.widget-section {
		flex: 0 0 auto;
		margin: 0 4px;
		margin-top: 4px;

		.header {
			text-align: left;
			align-items: center;
			display: flex;
			flex: 0 0 24px;
			padding-left: 8px;
			padding-right: 0;
			margin-bottom: 4px;
			border: 0;
			border-radius: 4px;
			background: var(--color-2-mildblack);

			&.expanded {
				border-radius: 4px 4px 0 0;
				margin-bottom: 0;

				.expand-arrow::after {
					transform: rotate(90deg);
				}
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

			.text-label {
				height: 18px;
				margin-left: 8px;
				flex: 1 1 100%;
			}
		}

		&:not(:hover) .header .show-only-on-hover {
			display: none;
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

<script lang="ts">
	import { getContext } from "svelte";
	import LayoutCol from "/src/components/layout/LayoutCol.svelte";
	import IconButton from "/src/components/widgets/buttons/IconButton.svelte";
	import TextLabel from "/src/components/widgets/labels/TextLabel.svelte";
	import WidgetSpan from "/src/components/widgets/WidgetSpan.svelte";
	import type { EditorWrapper, LayoutTarget, WidgetSection as WidgetSectionData } from "/wrapper/pkg/graphite_wasm_wrapper";

	export let widgetData: WidgetSectionData;
	export let layoutTarget: LayoutTarget;

	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};

	let expanded = true;

	const editor = getContext<EditorWrapper>("editor");
</script>

<!-- TODO: Implement collapsable sections with properties system -->
<LayoutCol class={`widget-section ${className}`.trim()} {classes}>
	<button class="header" class:expanded on:click|stopPropagation={() => (expanded = !expanded)} tabindex="0">
		<div class="expand-arrow"></div>
		<TextLabel tooltipLabel={widgetData.name} tooltipDescription={widgetData.description} bold={true}>{widgetData.name}</TextLabel>
		<IconButton
			icon={widgetData.pinned ? "PinActive" : "PinInactive"}
			tooltipDescription={widgetData.pinned ? "Unpin this node so it's no longer shown here when nothing is selected." : "Pin this node so it's shown here when nothing is selected."}
			size={24}
			action={(e) => {
				editor.setNodePinned(widgetData.id, !widgetData.pinned);
				e?.stopPropagation();
			}}
			class="show-only-on-hover"
		/>
		<IconButton
			icon="Trash"
			tooltipDescription="Delete this node from the layer chain."
			size={24}
			action={(e) => {
				editor.deleteNode(widgetData.id);
				e?.stopPropagation();
			}}
			class="show-only-on-hover"
		/>
		<IconButton
			icon={widgetData.visible ? "EyeVisible" : "EyeHidden"}
			hoverIcon={widgetData.visible ? "EyeHide" : "EyeShow"}
			tooltipDescription={widgetData.visible ? "Hide this node." : "Show this node."}
			size={24}
			action={(e) => {
				editor.toggleNodeVisibilityLayerPanel(widgetData.id);
				e?.stopPropagation();
			}}
			class={widgetData.visible ? "show-only-on-hover" : ""}
		/>
	</button>
	{#if expanded}
		<LayoutCol class="body" data-block-hover-transfer>
			{#each widgetData.layout as layoutGroup}
				{#if "Row" in layoutGroup}
					<WidgetSpan direction="row" widgets={layoutGroup.Row.rowWidgets} {layoutTarget} />
				{:else if "Section" in layoutGroup}
					<svelte:self widgetData={layoutGroup.Section} {layoutTarget} />
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

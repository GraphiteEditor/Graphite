<script lang="ts">
	import { getContext } from "svelte";
	import LayoutCol from "/src/components/layout/LayoutCol.svelte";
	import IconButton from "/src/components/widgets/buttons/IconButton.svelte";
	import IconLabel from "/src/components/widgets/labels/IconLabel.svelte";
	import TextLabel from "/src/components/widgets/labels/TextLabel.svelte";
	import WidgetSpan from "/src/components/widgets/WidgetSpan.svelte";
	import type { TooltipStore } from "/src/stores/tooltip";
	import { operatingSystem } from "/src/utility-functions/platform";
	import type { EditorWrapper, LayoutTarget, WidgetSection as WidgetSectionData } from "/wrapper/pkg/graphite_wasm_wrapper";

	export let widgetData: WidgetSectionData;
	export let layoutTarget: LayoutTarget;

	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};

	// Whether the section is expanded is owned by the backend (persisted per node), so just reflect it here
	$: expanded = widgetData.expanded;

	// The sections of the chain feeding one of this node's inputs, which follow its own rows and are shown beneath them
	$: chainSections = widgetData.layout.flatMap((layoutGroup) => ("Section" in layoutGroup && layoutGroup.Section.outputWire ? [layoutGroup.Section] : []));

	// A reorderable section is a Properties panel node section the user can drag to reorder (a layer chain's node, or a pinned node)
	$: reorderable = layoutTarget === "PropertiesPanel" && widgetData.draggable;

	const editor = getContext<EditorWrapper>("editor");
	const tooltip = getContext<TooltipStore>("tooltip");

	// Like the Layers panel's expand arrows, a modifier key applies the toggle recursively to the sections nested within
	function toggleExpandedWithModifiers(e: MouseEvent) {
		const accel = operatingSystem() === "Mac" ? e.metaKey : e.ctrlKey;
		editor.toggleNodePropertiesSectionExpanded(widgetData.id, e.altKey || accel);
	}
</script>

<!-- TODO: Implement collapsable sections with properties system -->
<LayoutCol
	class={`widget-section ${className}`.trim()}
	classes={{
		...classes,
		"chain-wire": Boolean(widgetData.chainWire),
		"chain-wire-list": Boolean(widgetData.chainWire?.isList),
		"output-wire": Boolean(widgetData.outputWire),
		"output-wire-list": Boolean(widgetData.outputWire?.isList),
		"node-reference": widgetData.nodeReference,
		collapsed: !expanded,
	}}
	styles={{
		"--chain-wire-color": widgetData.chainWire && `var(--color-data-${widgetData.chainWire.dataType.toLowerCase()}-dim)`,
		"--output-wire-color": widgetData.outputWire && `var(--color-data-${widgetData.outputWire.dataType.toLowerCase()}-dim)`,
	}}
	data-properties-reorderable-section={reorderable ? "" : undefined}
	data-node-id={reorderable ? String(widgetData.id) : undefined}
>
	{#if widgetData.nodeReference}
		{#each widgetData.layout as layoutGroup}
			{#if "Row" in layoutGroup}
				<WidgetSpan direction="row" widgets={layoutGroup.Row.rowWidgets} {layoutTarget} />
			{/if}
		{/each}
	{:else}
		<button
			class="header"
			class:expanded
			data-tooltip-label={widgetData.name}
			data-tooltip-description={widgetData.description}
			data-properties-reorder-handle={reorderable ? "" : undefined}
			on:click|stopPropagation={toggleExpandedWithModifiers}
			tabindex="0"
		>
			<div
				class="expand-arrow"
				data-tooltip-label={expanded ? "Collapse (All)" : "Expand (All)"}
				data-tooltip-description={expanded
					? "Hide this node's parameters. (To also collapse every node nested within, perform the shortcut shown.)"
					: "Show this node's parameters. (To also expand every node nested within, perform the shortcut shown.)"}
				data-tooltip-shortcut={$tooltip.altClickShortcut?.shortcut ? JSON.stringify($tooltip.altClickShortcut.shortcut) : undefined}
			></div>
			{#if widgetData.icon}
				<IconLabel icon={widgetData.icon} />
			{/if}
			<TextLabel bold={true}>{widgetData.name}</TextLabel>
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
				icon={widgetData.pinned ? "PinActive" : "PinInactive"}
				tooltipDescription={widgetData.pinned ? "Unpin this node so it's no longer shown here when nothing is selected." : "Pin this node so it's shown here when nothing is selected."}
				size={24}
				action={(e) => {
					editor.setNodePinned(widgetData.id, !widgetData.pinned);
					e?.stopPropagation();
				}}
				class={widgetData.pinned ? "" : "show-only-on-hover"}
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
					{:else if "Section" in layoutGroup && !layoutGroup.Section.outputWire}
						<svelte:self widgetData={layoutGroup.Section} {layoutTarget} />
					{/if}
				{/each}
			</LayoutCol>
			{#if chainSections.length > 0}
				<LayoutCol class="chain-sections">
					{#each chainSections as chainSection}
						<svelte:self widgetData={chainSection} {layoutTarget} />
					{/each}
				</LayoutCol>
			{/if}
		{/if}
	{/if}
</LayoutCol>

<style lang="scss">
	.widget-section {
		flex: 0 0 auto;
		margin: 0 4px;

		+ .widget-section {
			margin-top: 4px;
		}

		// A section showing the chain feeding one of its inputs, which it frames below its rows, with a wire rising from them through its body into that input
		&.chain-wire {
			--chain-wire-corner-radius: 4px;
			--chain-wire-width: 2px;
			--chain-section-indent: 8px;
			// Within the body's border, the wire's centerline rises in line with the indented chain sections' expand arrows
			--chain-wire-x: calc(var(--chain-section-indent) + 12px - 1px);

			// Like in the graph, a list's wire is a 1px line, 2px gap, and 1px line
			&.chain-wire-list {
				--chain-wire-width: 4px;
			}

			// The body continues down into the frame around the chain's sections
			> .body {
				position: relative;
				border-bottom: none;
				border-radius: 0;
				anchor-scope: --chain-input;

				// The connector of the input whose chain is shown, which the wire turns into
				> .widget-span.row:has(> .parameter-chain-toggle.expanded) > .parameter-expose-button {
					anchor-name: --chain-input;
				}
			}

			// The chain's sections, indented on the left and sharing the right edge, with the wire to the body crossing the gap atop the first
			> .chain-sections {
				padding: 4px 0 calc(var(--chain-section-indent) - 1px) calc(var(--chain-section-indent) - 1px);
				background: var(--color-3-darkgray);
				border: 1px solid var(--color-2-mildblack);
				border-top: none;
				border-radius: 0 0 4px 4px;

				// Over the frame's right border, so the shared edge is one line
				> .widget-section {
					margin-left: 0;
					margin-right: -1px;
				}

				// Keeps the frame below as thick as beside
				> .widget-section.collapsed:last-child > .header {
					margin-bottom: 0;
				}
			}

			// Outer and inner 1px lines, which touch as one solid line unless the wire is a list, rising from the body's bottom and turning at the connector's middle to end at its left
			> .body::before,
			> .body::after {
				content: "";
				position: absolute;
				box-sizing: border-box;
				right: anchor(--chain-input left);
				bottom: 0;
				border-left: 1px solid var(--chain-wire-color);
				border-top: 1px solid var(--chain-wire-color);
				pointer-events: none;
			}

			> .body::after {
				left: calc(var(--chain-wire-x) - var(--chain-wire-width) / 2);
				top: calc(anchor(--chain-input center) - var(--chain-wire-width) / 2);
				border-top-left-radius: var(--chain-wire-corner-radius);
			}

			// Inset to stay concentric with the outer line around the corner
			> .body::before {
				left: calc(var(--chain-wire-x) + var(--chain-wire-width) / 2 - 1px);
				top: calc(anchor(--chain-input center) + var(--chain-wire-width) / 2 - 1px);
				border-top-left-radius: max(0px, var(--chain-wire-corner-radius) - var(--chain-wire-width) + 1px);
			}
		}

		// A section in such a chain, with a wire rising across the gap to the section above, in line with the containing section's wire
		&.output-wire {
			--output-wire-width: 2px;
			position: relative;

			&.output-wire-list {
				--output-wire-width: 4px;
			}

			// Two 1px lines, like the layer section's wire
			&::before {
				content: "";
				position: absolute;
				box-sizing: border-box;
				left: calc(12px - var(--output-wire-width) / 2);
				top: -4px;
				width: var(--output-wire-width);
				height: 4px;
				border-left: 1px solid var(--output-wire-color);
				border-right: 1px solid var(--output-wire-color);
				pointer-events: none;
			}
		}

		// A collapsed section keeps its header's bottom margin, which the wire from the section below crosses too
		&.collapsed + .widget-section.output-wire::before {
			top: calc(-4px - 4px);
			height: calc(4px + 4px);
		}

		// The layer whose stack feeds the bottom of a chain, only referred to by a button like the Data panel's, its 16px icon centered on the wire reaching down to it
		&.node-reference {
			// As wide as its contents, unlike the Properties panel's buttons that share their row
			.text-button {
				flex-basis: auto;
				padding-left: calc(12px - 16px / 2);
			}

			// Down to the 24px button centered in its 32px row
			&::before {
				height: calc(4px + (32px - 24px) / 2);
			}
		}

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

				+ .body,
				~ .chain-sections {
					border-color: var(--color-4-dimgray);
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

			> .icon-label {
				margin-left: 8px;
			}

			// As wide as the name, with the buttons pushed to the end
			.text-label {
				height: 18px;
				margin-left: 8px;
				margin-right: auto;
				flex: 0 1 auto;
				overflow: hidden;
				text-overflow: ellipsis;
			}
		}

		// Hovering the chain sections within doesn't count
		&:not(:hover) > .header .show-only-on-hover,
		&:has(> .chain-sections:hover) > .header .show-only-on-hover {
			display: none;
		}

		> .body {
			padding: 0 7px;
			padding-top: 1px;
			margin-top: -1px;
			background: var(--color-3-darkgray);
			border: 1px solid var(--color-2-mildblack);
			border-radius: 0 0 4px 4px;
			overflow: hidden;

			> .widget-span.row {
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

				// Indented to leave room for the wire that rises past to the connector of an input whose chain is shown
				> .parameter-expose-button:first-child {
					margin-left: 20px;

					// Shortened by the indent so the widgets after it stay aligned with other rows
					+ .text-label {
						flex-basis: calc(25% - 20px);
					}
				}

				// With no value widget after it, a chain toggle takes the rest of the row, only shrinking once that runs out
				> .text-label.parameter-chain-toggle:first-of-type {
					flex-grow: 1;
					flex-shrink: 1;
					min-width: 0;
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

				> .parameter-expose-button + .text-label ~ .number-input:last-child {
					margin-left: auto;
				}
			}
		}
	}
</style>

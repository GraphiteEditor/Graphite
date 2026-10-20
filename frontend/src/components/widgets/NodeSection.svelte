<script lang="ts">
	import { getContext } from "svelte";
	import LayoutCol from "/src/components/layout/LayoutCol.svelte";
	import IconButton from "/src/components/widgets/buttons/IconButton.svelte";
	import IconLabel from "/src/components/widgets/labels/IconLabel.svelte";
	import TextLabel from "/src/components/widgets/labels/TextLabel.svelte";
	import WidgetSpan from "/src/components/widgets/WidgetSpan.svelte";
	import type { TooltipStore } from "/src/stores/tooltip";
	import { operatingSystem } from "/src/utility-functions/platform";
	import type { EditorWrapper, LayoutTarget, NodeSection as NodeSectionData } from "/wrapper/pkg/graphite_wasm_wrapper";

	export let widgetData: NodeSectionData;
	export let layoutTarget: LayoutTarget;

	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};

	// A node without parameters has nothing to show, so its section stays collapsed with its arrow grayed out, like the Layers panel's
	$: expandable = widgetData.layout.length > 0;

	// Whether the section is expanded is owned by the backend (persisted per node), so just reflect it here
	$: expanded = widgetData.expanded && expandable;

	// The sections of the chain feeding one of this node's inputs, which follow its own rows and are shown beneath them
	$: chainSections = widgetData.layout.flatMap((layoutGroup) => ("NodeSection" in layoutGroup && layoutGroup.NodeSection.outputWire ? [layoutGroup.NodeSection] : []));

	// A reorderable section is a Inspector node section the user can drag to reorder (a layer chain's node, or a pinned node)
	$: reorderable = layoutTarget === "Inspector" && widgetData.draggable;

	const editor = getContext<EditorWrapper>("editor");
	const tooltip = getContext<TooltipStore>("tooltip");

	// Like the Layers panel's expand arrows, a modifier key applies the toggle recursively to the sections nested within
	function toggleExpandedWithModifiers(e: MouseEvent) {
		if (!expandable) return;

		const accel = operatingSystem() === "Mac" ? e.metaKey : e.ctrlKey;
		editor.toggleNodeInspectorSectionExpanded(widgetData.id, e.altKey || accel);
	}
</script>

<!-- TODO: Implement collapsable sections with the Inspector system -->
<LayoutCol
	class={`node-section ${className}`.trim()}
	classes={{
		...classes,
		"chain-wire": Boolean(widgetData.chainWire),
		"chain-wire-list": Boolean(widgetData.chainWire?.isList),
		"output-wire": Boolean(widgetData.outputWire),
		"output-wire-list": Boolean(widgetData.outputWire?.isList),
		"node-reference": widgetData.nodeReference,
		"content-wire": Boolean(widgetData.contentWire),
		"content-wire-list": Boolean(widgetData.contentWire?.isList),
		"content-wire-continues": widgetData.contentWireContinues,
		collapsed: !expanded,
	}}
	styles={{
		"--chain-wire-color": widgetData.chainWire && `var(--color-data-${widgetData.chainWire.dataType.toLowerCase()}-dim)`,
		"--output-wire-color": widgetData.outputWire && `var(--color-data-${widgetData.outputWire.dataType.toLowerCase()}-dim)`,
		"--content-wire-color": widgetData.contentWire && `var(--color-data-${widgetData.contentWire.dataType.toLowerCase()}-dim)`,
	}}
	data-inspector-reorderable-section={reorderable ? "" : undefined}
	data-node-id={reorderable ? String(widgetData.id) : undefined}
>
	{#if widgetData.nodeReference}
		{#if widgetData.contentWireContinues}
			<div class="content-wire-continuation"></div>
		{/if}
		{#each widgetData.layout as layoutGroup}
			{#if "Row" in layoutGroup}
				<WidgetSpan direction="row" widgets={layoutGroup.Row.rowWidgets} {layoutTarget} />
			{/if}
		{/each}
	{:else}
		<button class="header" class:expanded class:expandable data-inspector-reorder-handle={reorderable ? "" : undefined} on:click|stopPropagation={toggleExpandedWithModifiers}>
			<div
				class="icon-slot"
				data-tooltip-label={!expandable ? undefined : expanded ? "Collapse (All)" : "Expand (All)"}
				data-tooltip-description={!expandable
					? undefined
					: expanded
						? "Hide this node's parameters. (To also collapse every node nested within, perform the shortcut shown.)"
						: "Show this node's parameters. (To also expand every node nested within, perform the shortcut shown.)"}
				data-tooltip-shortcut={expandable && $tooltip.altClickShortcut?.shortcut ? JSON.stringify($tooltip.altClickShortcut.shortcut) : undefined}
			>
				{#if widgetData.icon}
					<IconLabel icon={widgetData.icon} />
				{/if}
				<div class="expand-arrow"></div>
			</div>
			<TextLabel tooltipLabel={widgetData.name} tooltipDescription={widgetData.description}>{widgetData.name}</TextLabel>
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
					editor.toggleNodeVisibilityInspector(widgetData.id);
					e?.stopPropagation();
				}}
				class={widgetData.visible ? "show-only-on-hover" : ""}
			/>
			{#if reorderable}
				<div class="drag-grip" data-tooltip-description="Drag to reorder this node." on:click|stopPropagation={() => undefined} role="presentation"></div>
			{/if}
		</button>
		{#if expanded}
			<LayoutCol class="body" data-block-hover-transfer data-section-contents>
				{#each widgetData.layout as layoutGroup}
					{#if "Row" in layoutGroup}
						<WidgetSpan direction="row" widgets={layoutGroup.Row.rowWidgets} {layoutTarget} />
					{:else if "NodeSection" in layoutGroup && !layoutGroup.NodeSection.outputWire}
						<svelte:self widgetData={layoutGroup.NodeSection} {layoutTarget} />
					{/if}
				{/each}
			</LayoutCol>
			{#if chainSections.length > 0}
				<LayoutCol class="chain-sections" data-section-contents>
					{#each chainSections as chainSection}
						<svelte:self widgetData={chainSection} {layoutTarget} />
					{/each}
				</LayoutCol>
			{/if}
		{/if}
	{/if}
</LayoutCol>

<style lang="scss">
	.node-section {
		--section-outline: var(--color-2-mildblack);
		// Where the icon is centered, and the wire running down through the sections: 4px inside the 1px outline, plus half the 16px icon
		--wire-center: calc(1px + 4px + 16px / 2);
		// Between neighboring sections, which the wires joining them cross
		--section-gap: 8px;
		flex: 0 0 auto;
		margin: 0 4px;

		+ .node-section {
			margin-top: var(--section-gap);
		}

		// Hovered over its header or parameters (but not a section nested among them, which reacts instead), its outline lightens and its icon becomes the expand arrow.
		// The chain shown below doesn't count, so passing between its sections leaves this one alone.
		&:has(> .header:hover, > .body:hover):not(:has(.node-section:hover)) {
			--section-outline: var(--color-4-dimgray);

			> .header .icon-slot {
				.icon-label {
					display: none;
				}

				.expand-arrow {
					display: flex;
				}
			}
		}

		// A section showing the chain feeding one of its inputs, which it frames below its rows, with a wire rising from them through its body into that input
		&.chain-wire {
			--chain-wire-corner-radius: 4px;
			--chain-wire-width: 2px;
			--chain-section-indent: 8px;
			// Within the body's border, the wire's centerline rises in line with the indented chain sections' icons
			--chain-wire-x: calc(var(--chain-section-indent) + var(--wire-center) - 1px);

			// Like in the graph, a list's wire is a 1px line, 2px gap, and 1px line
			&.chain-wire-list {
				--chain-wire-width: 4px;
			}

			// The body continues down into the frame around the chain's sections, the first right beneath its rows
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
				padding: 0 0 calc(var(--chain-section-indent) - 1px) calc(var(--chain-section-indent) - 1px);
				background: var(--color-3-darkgray);
				border: 1px solid var(--section-outline);
				border-top: none;
				border-radius: 0 0 4px 4px;

				// Over the frame's right border, so the shared edge is one line
				> .node-section {
					margin-left: 0;
					margin-right: -1px;
				}

				// With no gap above the first to cross, the body's wire meets it directly
				> .node-section:first-child::before {
					display: none;
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
				left: calc(var(--wire-center) - var(--output-wire-width) / 2);
				top: calc(-1 * var(--section-gap));
				width: var(--output-wire-width);
				height: var(--section-gap);
				border-left: 1px solid var(--output-wire-color);
				border-right: 1px solid var(--output-wire-color);
				pointer-events: none;
			}
		}

		// A layer only referred to by a button like the Data panel's, its 16px icon centered on the wire reaching it
		&.node-reference {
			// Only as tall as the button, so it's spaced from the sections around it like they are from each other
			> .widget-span.row {
				--row-height: 24px;
			}

			// As wide as its contents, unlike the Inspector's buttons that share their row, and flush within an outline like the sections'
			.text-button {
				flex-basis: auto;
				padding-left: calc(var(--wire-center) - 16px / 2);
				--button-background-color: none;
				box-shadow: inset 0 0 0 1px var(--section-outline);

				&:hover:not(.disabled) {
					--button-background-color: var(--color-5-dullgray);
					--section-outline: var(--color-4-dimgray);
				}
			}
		}

		// Layers taking the wire rising from the section below into their content, indented so it turns in from the left at the middle of their buttons
		&.content-wire {
			--content-wire-width: 2px;
			--content-wire-indent: 20px;
			position: relative;
			padding-left: var(--content-wire-indent);

			&.content-wire-list {
				--content-wire-width: 4px;
			}

			// Outer and inner 1px lines, rising from the bottom and turning toward the buttons, like the wire into a shown chain's input
			&::before,
			&::after {
				content: "";
				position: absolute;
				box-sizing: border-box;
				right: calc(100% - var(--content-wire-indent));
				bottom: 0;
				border-left: 1px solid var(--content-wire-color);
				border-top: 1px solid var(--content-wire-color);
				pointer-events: none;
			}

			&::after {
				left: calc(var(--wire-center) - var(--content-wire-width) / 2);
				top: calc(50% - var(--content-wire-width) / 2);
				border-top-left-radius: 4px;
			}

			&::before {
				left: calc(var(--wire-center) + var(--content-wire-width) / 2 - 1px);
				top: calc(50% + var(--content-wire-width) / 2 - 1px);
				border-top-left-radius: max(0px, 4px - var(--content-wire-width) + 1px);
			}

			// Also continuing straight up past the turn into the bottom of the buttons above, beneath the turn, which hides where it crosses
			&.content-wire-continues {
				.content-wire-continuation {
					position: absolute;
					box-sizing: border-box;
					left: calc(var(--wire-center) - var(--content-wire-width) / 2);
					width: var(--content-wire-width);
					top: calc(-1 * var(--section-gap));
					bottom: 0;
					border-left: 1px solid var(--content-wire-color);
					border-right: 1px solid var(--content-wire-color);
					pointer-events: none;
				}

				&::after {
					background: var(--color-3-darkgray);
					z-index: 1;
				}

				&::before {
					z-index: 2;
				}
			}
		}

		.header {
			text-align: left;
			align-items: center;
			display: flex;
			flex: 0 0 24px;
			padding: 0;
			border: 0;
			border-radius: 4px;
			background: none;
			// The section's outline, continuing up around the header without changing its size
			box-shadow: inset 0 0 0 1px var(--section-outline);

			&.expanded {
				border-radius: 4px 4px 0 0;
				// Open at the bottom, where the body's sides carry on
				box-shadow:
					inset 1px 0 0 var(--section-outline),
					inset -1px 0 0 var(--section-outline),
					inset 0 1px 0 var(--section-outline);

				.expand-arrow::after {
					transform: rotate(90deg);
				}
			}

			&.expandable:hover .expand-arrow::after {
				background: var(--icon-expand-collapse-arrow-hover);
			}

			&:not(.expandable) .expand-arrow::after {
				background: var(--icon-expand-collapse-arrow-disabled);
			}

			// Centers the icon on the wire, swapping in the expand arrow while the section is hovered
			.icon-slot {
				flex: 0 0 24px;
				height: 24px;
				margin-left: calc(var(--wire-center) - 24px / 2);
				display: flex;
				align-items: center;
				justify-content: center;

				.expand-arrow {
					display: none;
				}
			}

			.expand-arrow {
				width: 8px;
				height: 8px;
				margin: 0;
				padding: 0;
				position: relative;
				flex: 0 0 auto;
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

			// Like a graph layer's grip
			.drag-grip {
				flex: 0 0 auto;
				width: 8px;
				height: 24px;
				// Padding rather than margin, so the gaps count toward the tooltip's hover area
				padding: 0 4px;
				background-image: var(--icon-drag-grip);
				background-position: calc(4px + 2px) 8px;
				background-repeat: no-repeat;

				&:hover {
					background-image: var(--icon-drag-grip-hover);
				}
			}

			// As wide as the name, with the buttons pushed to the end
			.text-label {
				height: 18px;
				margin-left: 4px;
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
			border: 1px solid var(--section-outline);
			// No line between it and the header, which shares its outline
			border-top-color: transparent;
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
					flex: 0 1 calc(20% + var(--absorbed-room, 0px) - var(--wire-indent, 0px));
					margin-left: 16px;

					// A parameter name keeps its column until the widgets after it are squeezed to a usable width, then gives up space and fades where it's cut off
					&:not(:last-child, .parameter-chain-toggle) {
						overflow: hidden;
						mask-image: linear-gradient(to left, transparent, black 12px);
					}
				}

				// A blank assist and the gap after it become room for the name
				> .text-label:first-of-type:has(+ .separator.section + .separator.unrelated) {
					--absorbed-room: calc(24px + 16px);

					+ .separator.section,
					+ .separator.section + .separator.unrelated {
						display: none;
					}
				}

				// An optional parameter's checkbox fills the assist, so only the gap before it becomes room for the name
				> .text-label:first-of-type:has(+ .separator.unrelated + .separator.related) {
					--absorbed-room: 16px;

					+ .separator.unrelated {
						display: none;
					}
				}

				// Indented to leave room for the wire that rises past to the connector of an input whose chain is shown
				> .parameter-expose-button:first-child {
					margin-left: 20px;

					// Shortened by the indent so the widgets after it stay aligned with other rows
					+ .text-label {
						--wire-indent: 20px;
					}
				}

				// A chain toggle is only as wide as its text, so the rest of the row isn't clickable, only shrinking once the row runs out
				> .text-label.parameter-chain-toggle:first-of-type {
					flex: 0 1 auto;
					min-width: 0;
				}

				> .parameter-expose-button + .text-label:first-of-type {
					margin-left: 8px;
				}

				// A label alone in its row lines up with the parameter names that follow their expose buttons
				> .text-label:only-child {
					margin-left: calc(20px + 8px + 8px);
				}

				> .separator.horizontal.unrelated {
					flex-shrink: 1;
					min-width: 8px;
				}

				// Widgets share the row's width, each narrowing to no less than a usable width
				> :is(.number-input, .text-input, .dropdown-input, .color-input, .text-button, .radio-input, .slider-input) {
					flex: 1 1 0;
				}

				> :is(.number-input, .text-input, .dropdown-input, .color-input, .text-button) {
					min-width: 64px;
				}

				> .slider-input {
					min-width: 32px;
				}

				> .parameter-expose-button + .text-label ~ .number-input:last-child {
					margin-left: auto;
				}
			}
		}
	}
</style>

<script lang="ts">
	import LayoutRow from "/src/components/layout/LayoutRow.svelte";
	import { consumeConnectorClickSuppression, hoveredWireDropTarget, pressPropertiesPanelConnector } from "/src/utility-functions/wire-drag";
	import type { FrontendGraphDataType, ActionShortcut } from "/wrapper/pkg/graphite_wasm_wrapper";

	// Content
	export let exposed: boolean;
	export let dataType: FrontendGraphDataType;
	export let nodeId: bigint;
	export let inputIndex: number;
	export let wireDropTarget: boolean;
	// Tooltips
	export let tooltipLabel: string | undefined = undefined;
	export let tooltipDescription: string | undefined = undefined;
	export let tooltipShortcut: ActionShortcut | undefined = undefined;
	// Callbacks
	export let action: (e?: MouseEvent) => void;

	$: wireHovering = wireDropTarget && $hoveredWireDropTarget?.nodeId === nodeId && $hoveredWireDropTarget?.inputIndex === inputIndex;

	function click(e: MouseEvent) {
		// A press that turned into dragging a wire out of this connector shouldn't also toggle its exposure
		if (consumeConnectorClickSuppression()) return;

		action(e);
	}
</script>

<LayoutRow class="parameter-expose-button">
	<button
		class:exposed
		class:wire-drop-target={wireDropTarget}
		class:wire-hovering={wireHovering}
		style:--data-type-color={`var(--color-data-${dataType.toLowerCase()})`}
		style:--data-type-color-dim={`var(--color-data-${dataType.toLowerCase()}-dim)`}
		on:pointerdown={(e) => pressPropertiesPanelConnector(e, { nodeId, inputIndex })}
		on:click={click}
		data-wire-drop-target={wireDropTarget || undefined}
		data-node-id={String(nodeId)}
		data-input-index={inputIndex}
		data-tooltip-label={tooltipLabel}
		data-tooltip-description={tooltipDescription}
		data-tooltip-shortcut={tooltipShortcut?.shortcut ? JSON.stringify(tooltipShortcut.shortcut) : undefined}
		tabindex="-1"
	>
		{#if exposed || wireHovering}
			<!-- Same shape as an input connector in the node graph -->
			<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8">
				<path class="interior" d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" />
			</svg>
		{:else}
			<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8">
				<circle class="interior" r="3" cx="4" cy="4" />
				<path
					class="outline"
					d="M4,1C5.656,1 7,2.344 7,4C7,5.656 5.656,7 4,7C2.344,7 1,5.656 1,4C1,2.344 2.344,1 4,1ZM4,2C2.896,2 2,2.896 2,4C2,5.104 2.896,6 4,6C5.104,6 6,5.104 6,4C6,2.896 5.104,2 4,2z"
				/>
			</svg>
		{/if}
	</button>
</LayoutRow>

<style lang="scss">
	.parameter-expose-button {
		display: flex;
		align-items: center;
		flex: 0 0 auto;
		max-height: 24px;

		button {
			position: relative;
			flex: 0 0 auto;
			width: 8px;
			height: 8px;
			margin: 0;
			padding: 0;
			border: none;
			background: none;
			fill: none;
			stroke: none;

			// Enlarges the area for grabbing the connector or dropping a wire onto it, without affecting the row layout
			&::before {
				content: "";
				position: absolute;
				inset: -4px;
			}

			svg {
				display: block;
				overflow: visible;
			}

			.outline {
				fill: none;
			}

			.interior {
				fill: var(--data-type-color);
			}

			&:hover:not(.exposed) {
				.outline {
					fill: var(--data-type-color);
				}

				.interior {
					fill: var(--data-type-color-dim);
				}
			}

			&.exposed:hover {
				.interior {
					fill: var(--data-type-color-dim);
					stroke: var(--data-type-color);
					stroke-width: 1px;
					vector-effect: non-scaling-stroke;
				}
			}

			// Rings each connector that the wire being dragged could be dropped onto
			&.wire-drop-target::after {
				content: "";
				position: absolute;
				inset: -4px;
				border: 1px solid var(--data-type-color);
				border-radius: 50%;
				pointer-events: none;
			}

			// Previews the connection the dragged wire would make if dropped here
			&.wire-hovering .interior {
				fill: var(--data-type-color);
				stroke: var(--color-f-white);
				stroke-width: 1px;
				vector-effect: non-scaling-stroke;
			}
		}
	}
</style>

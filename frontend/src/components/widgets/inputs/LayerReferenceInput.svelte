<script lang="ts">
	import { createEventDispatcher } from "svelte";

	import { currentDraggingElement } from "~/src/io-managers/drag";

	import type { LayerType, LayerTypeData } from "~/src/wasm-communication/messages";
	import { layerTypeData } from "~/src/wasm-communication/messages";

	import LayoutRow from "~/src/components/layout/LayoutRow.svelte";
	import IconButton from "~/src/components/widgets/buttons/IconButton.svelte";
	import IconLabel from "~/src/components/widgets/labels/IconLabel.svelte";
	import TextLabel from "~/src/components/widgets/labels/TextLabel.svelte";

	// emits: ["update:value"],
	const dispatch = createEventDispatcher<{ value: string | undefined }>();

	export let value: string | undefined = undefined;
	export let layerName: string | undefined = undefined;
	export let layerType: LayerType | undefined = undefined;
	export let disabled = false;
	export let tooltip: string | undefined = undefined;
	export let sharpRightCorners = false;

	let hoveringDrop = false;

	$: droppable = hoveringDrop && Boolean(currentDraggingElement());

	function dragOver(e: DragEvent): void {
		hoveringDrop = true;

		e.preventDefault();
	}

	function drop(e: DragEvent): void {
		hoveringDrop = false;

		const element = currentDraggingElement();
		const layerPath = element?.getAttribute("data-layer") || undefined;

		if (layerPath) {
			e.preventDefault();

			dispatch("value", layerPath);
		}
	}

	function getLayerTypeData(layerType: LayerType): LayerTypeData {
		return layerTypeData(layerType) || { name: "Error", icon: "Info" };
	}
</script>

<LayoutRow
	class="layer-reference-input"
	classes={{ disabled, droppable, "sharp-right-corners": sharpRightCorners }}
	{tooltip}
	on:dragover={(e) => !disabled && dragOver(e)}
	on:dragleave={() => !disabled && (hoveringDrop = false)}
	on:drop={(e) => !disabled && drop(e)}
>
	{#if value === undefined || droppable}
		<LayoutRow class="drop-zone" />
		<TextLabel italic={true}>{droppable ? "Drop" : "Drag"} Layer Here</TextLabel>
	{:else}
		{#if layerName !== undefined && layerType}
			<IconLabel icon={getLayerTypeData(layerType).icon} class="layer-icon" />
			<TextLabel italic={layerName === ""} class="layer-name">{layerName || getLayerTypeData(layerType).name}</TextLabel>
		{:else}
			<TextLabel bold={true} italic={true} class="missing">Layer Missing</TextLabel>
		{/if}
		<IconButton icon="CloseX" size={16} {disabled} action={() => dispatch("value", undefined)} />
	{/if}
</LayoutRow>

<style lang="scss" global>
	.layer-reference-input {
		position: relative;
		flex: 1 0 auto;
		height: 24px;
		border-radius: 2px;
		background: var(--color-1-nearblack);

		.drop-zone {
			pointer-events: none;
			border: 1px dashed var(--color-5-dullgray);
			border-radius: 1px;
			position: absolute;
			top: 2px;
			bottom: 2px;
			left: 2px;
			right: 2px;
		}

		&.droppable .drop-zone {
			border: 1px dashed var(--color-e-nearwhite);
		}

		.layer-icon {
			margin: 4px 8px;

			+ .text-label {
				padding-left: 0;
			}
		}

		.text-label {
			line-height: 18px;
			padding: 3px calc(8px + 2px);
			width: 100%;
			text-align: center;

			&.missing {
				// TODO: Define this as a permanent color palette choice (search the project for all uses of this hex code)
				color: #d6536e;
			}

			&.layer-name {
				text-align: left;
			}
		}

		.icon-button {
			margin: 4px;
			margin-left: 0;
		}

		&.disabled {
			background: var(--color-2-mildblack);

			.drop-zone {
				border: 1px dashed var(--color-4-dimgray);
			}

			.text-label {
				color: var(--color-8-uppergray);
			}

			.icon-label svg {
				fill: var(--color-8-uppergray);
			}
		}
	}
</style>

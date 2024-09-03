<script lang="ts">
	import { getContext, tick } from "svelte";

	import { beginDraggingElement } from "@graphite/io-managers/drag";
	import type { LayerListingInfo, LayersState } from "@graphite/state-providers/layers";
	import type { NodeGraphState } from "@graphite/state-providers/node-graph";
	import { platformIsMac } from "@graphite/utility-functions/platform";
	import type { Editor } from "@graphite/wasm-communication/editor";

	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import IconButton from "@graphite/components/widgets/buttons/IconButton.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";
	import WidgetLayout from "@graphite/components/widgets/WidgetLayout.svelte";

	type DraggingData = {
		select?: () => void;
		insertParentId: bigint | undefined;
		insertDepth: number;
		insertIndex: number | undefined;
		highlightFolder: boolean;
		markerHeight: number;
	};

	const editor = getContext<Editor>("editor");
	const nodeGraph = getContext<NodeGraphState>("nodeGraph");
	const layersState = getContext<LayersState>("layers");

	let list: LayoutCol | undefined;

	// Interactive dragging
	let draggable = true;
	let draggingData: undefined | DraggingData = undefined;
	let fakeHighlight: undefined | bigint = undefined;
	let dragInPanel = false;
	let isDraggingLayer = false;

	function toggleNodeVisibilityLayerPanel(id: bigint) {
		editor.handle.toggleNodeVisibilityLayerPanel(id);
	}

	function toggleLayerLock(id: bigint) {
		editor.handle.toggleLayerLock(id);
	}

	function handleExpandArrowClick(id: bigint) {
		editor.handle.toggleLayerExpansion(id);
	}

	async function onEditLayerName(listing: LayerListingInfo) {
		if (listing.editingName) return;

		draggable = false;
		listing.editingName = true;

		await tick();

		const query = list?.div?.()?.querySelector("[data-text-input]:not([disabled])");
		const textInput = (query instanceof HTMLInputElement && query) || undefined;
		textInput?.select();
	}

	function onEditLayerNameChange(listing: LayerListingInfo, e: Event) {
		// Eliminate duplicate events
		if (!listing.editingName) return;

		draggable = true;
		listing.editingName = false;

		const name = (e.target instanceof HTMLInputElement && e.target.value) || "";
		editor.handle.setLayerName(listing.entry.id, name);
		listing.entry.alias = name;
	}

	async function onEditLayerNameDeselect(listing: LayerListingInfo) {
		draggable = true;
		listing.editingName = false;

		// Set it back to the original name if the user didn't enter a new name
		if (document.activeElement instanceof HTMLInputElement) document.activeElement.value = listing.entry.alias;

		// Deselect the text so it doesn't appear selected while the input field becomes disabled and styled to look like regular text
		window.getSelection()?.removeAllRanges();
	}

	function selectLayerWithModifiers(e: MouseEvent, listing: LayerListingInfo) {
		// Get the pressed state of the modifier keys
		const [ctrl, meta, shift, alt] = [e.ctrlKey, e.metaKey, e.shiftKey, e.altKey];
		// Get the state of the platform's accel key and its opposite platform's accel key
		const [accel, oppositeAccel] = platformIsMac() ? [meta, ctrl] : [ctrl, meta];

		// Select the layer only if the accel and/or shift keys are pressed
		if (!oppositeAccel && !alt) selectLayer(listing, accel, shift);

		e.stopPropagation();
	}

	function selectLayer(listing: LayerListingInfo, accel: boolean, shift: boolean) {
		// Don't select while we are entering text to rename the layer
		if (listing.editingName) return;

		editor.handle.selectLayer(listing.entry.id, accel, shift);
	}

	async function deselectAllLayers() {
		editor.handle.deselectAllLayers();
	}

	function calculateDragIndex(tree: LayoutCol, clientY: number, select?: () => void): DraggingData {
		const treeChildren = tree.div()?.children;
		const treeOffset = tree.div()?.getBoundingClientRect().top;

		// Folder to insert into
		let insertParentId: bigint | undefined = undefined;
		let insertDepth = 0;

		// Insert index (starts at the end, essentially infinity)
		let insertIndex = undefined;

		// Whether you are inserting into a folder and should show the folder outline
		let highlightFolder = false;

		let markerHeight = 0;
		const layerPanel = document.querySelector("[data-layer-panel]"); // Selects the element with the data-layer-panel attribute
		if (layerPanel !== null && treeChildren !== undefined && treeOffset !== undefined) {
			let layerPanelTop = layerPanel.getBoundingClientRect().top;
			Array.from(treeChildren).forEach((treeChild) => {
				const indexAttribute = treeChild.getAttribute("data-index");
				if (!indexAttribute) return;
				const { folderIndex, entry: layer } = $layersState.layers[parseInt(indexAttribute, 10)];

				const rect = treeChild.getBoundingClientRect();
				if (rect.top > clientY || rect.bottom < clientY) {
					return;
				}
				const pointerPercentage = (clientY - rect.top) / rect.height;
				if (layer.childrenAllowed) {
					if (pointerPercentage < 0.25) {
						insertParentId = layer.parentId;
						insertDepth = layer.depth - 1;
						insertIndex = folderIndex;
						markerHeight = rect.top - layerPanelTop;
					} else if (pointerPercentage < 0.75 || (layer.childrenPresent && layer.expanded)) {
						insertParentId = layer.id;
						insertDepth = layer.depth;
						insertIndex = 0;
						highlightFolder = true;
					} else {
						insertParentId = layer.parentId;
						insertDepth = layer.depth - 1;
						insertIndex = folderIndex + 1;
						markerHeight = rect.bottom - layerPanelTop;
					}
				} else {
					if (pointerPercentage < 0.5) {
						insertParentId = layer.parentId;
						insertDepth = layer.depth - 1;
						insertIndex = folderIndex;
						markerHeight = rect.top - layerPanelTop;
					} else {
						insertParentId = layer.parentId;
						insertDepth = layer.depth - 1;
						insertIndex = folderIndex + 1;
						markerHeight = rect.bottom - layerPanelTop;
					}
				}
			});
			// Dragging to the empty space below all layers
			let lastLayer = treeChildren[treeChildren.length - 1];
			if (lastLayer.getBoundingClientRect().bottom < clientY) {
				const numberRootLayers = $layersState.layers.filter((layer) => layer.entry.depth === 1).length;
				insertParentId = undefined;
				insertDepth = 0;
				insertIndex = numberRootLayers;
				markerHeight = lastLayer.getBoundingClientRect().bottom - layerPanelTop;
			}
		}

		return {
			select,
			insertParentId,
			insertDepth,
			insertIndex,
			highlightFolder,
			markerHeight,
		};
	}

	async function dragStart(event: DragEvent, listing: LayerListingInfo) {
		const layer = listing.entry;
		dragInPanel = true;
		if (!$nodeGraph.selected.includes(layer.id)) {
			fakeHighlight = layer.id;
		}
		const select = () => {
			if (!$nodeGraph.selected.includes(layer.id)) selectLayer(listing, false, false);
		};

		const target = (event.target instanceof HTMLElement && event.target) || undefined;
		const closest = target?.closest("[data-layer]") || undefined;
		const draggingELement = (closest instanceof HTMLElement && closest) || undefined;
		if (draggingELement) beginDraggingElement(draggingELement);

		// Set style of cursor for drag
		if (event.dataTransfer) {
			event.dataTransfer.dropEffect = "move";
			event.dataTransfer.effectAllowed = "move";
		}

		if (list) draggingData = calculateDragIndex(list, event.clientY, select);

		isDraggingLayer = true;
	}

	function updateInsertLine(event: DragEvent) {
		if (!isDraggingLayer) return;

		// Stop the drag from being shown as cancelled
		event.preventDefault();
		dragInPanel = true;

		if (list) draggingData = calculateDragIndex(list, event.clientY, draggingData?.select);
	}

	async function drop() {
		if (!isDraggingLayer) return;

		if (draggingData && dragInPanel) {
			const { select, insertParentId, insertIndex } = draggingData;

			select?.();
			editor.handle.moveLayerInTree(insertParentId, insertIndex);
		}
		draggingData = undefined;
		fakeHighlight = undefined;
		dragInPanel = false;
		isDraggingLayer = false;
	}
</script>

<LayoutCol class="layers" on:dragleave={() => (dragInPanel = false) && (isDraggingLayer = false)}>
	<LayoutRow class="options-bar" scrollableX={true}>
		<WidgetLayout layout={$layersState.layersPanelOptionsLayout} />
	</LayoutRow>
	<LayoutRow class="list-area" scrollableY={true}>
		<LayoutCol class="list" data-layer-panel bind:this={list} on:click={() => deselectAllLayers()} on:dragover={(e) => draggable && updateInsertLine(e)} on:dragend={() => draggable && drop()}>
			{#each $layersState.layers as listing, index}
				<LayoutRow
					class="layer"
					classes={{
						selected: fakeHighlight !== undefined ? fakeHighlight === listing.entry.id : listing.entry.selected,
						"full-highlight": listing.entry.inSelectedNetwork && !listing.entry.selectedParent,
						"insert-folder": (draggingData?.highlightFolder || false) && draggingData?.insertParentId === listing.entry.id,
					}}
					styles={{ "--layer-indent-levels": `${listing.entry.depth - 1}` }}
					data-layer
					data-index={index}
					tooltip={listing.entry.tooltip}
					{draggable}
					on:dragstart={(e) => draggable && dragStart(e, listing)}
					on:click={(e) => selectLayerWithModifiers(e, listing)}
				>
					{#if listing.entry.childrenAllowed}
						<button
							class="expand-arrow"
							class:expanded={listing.entry.expanded}
							disabled={!listing.entry.childrenPresent}
							on:click|stopPropagation={() => handleExpandArrowClick(listing.entry.id)}
							tabindex="0"
						/>
					{/if}
					<div class="thumbnail">
						{#if $nodeGraph.thumbnails.has(listing.entry.id)}
							{@html $nodeGraph.thumbnails.get(listing.entry.id)}
						{/if}
					</div>
					{#if listing.entry.name === "Artboard"}
						<IconLabel icon="Artboard" class={"layer-type-icon"} />
					{/if}
					<LayoutRow class="layer-name" on:dblclick={() => onEditLayerName(listing)}>
						<input
							data-text-input
							type="text"
							value={listing.entry.alias}
							placeholder={listing.entry.name}
							disabled={!listing.editingName}
							on:blur={() => onEditLayerNameDeselect(listing)}
							on:keydown={(e) => e.key === "Escape" && onEditLayerNameDeselect(listing)}
							on:keydown={(e) => e.key === "Enter" && onEditLayerNameChange(listing, e)}
							on:change={(e) => onEditLayerNameChange(listing, e)}
						/>
					</LayoutRow>
					{#if !listing.entry.unlocked || !listing.entry.parentsUnlocked}
						<IconButton
							class={"status-toggle"}
							classes={{ inactive: !listing.entry.parentsUnlocked }}
							action={(e) => (toggleLayerLock(listing.entry.id), e?.stopPropagation())}
							size={24}
							icon={listing.entry.unlocked ? "PadlockUnlocked" : "PadlockLocked"}
							hoverIcon={listing.entry.unlocked ? "PadlockLocked" : "PadlockUnlocked"}
							tooltip={listing.entry.unlocked ? "Lock" : "Unlock"}
						/>
					{/if}
					<IconButton
						class={"status-toggle"}
						classes={{ inactive: !listing.entry.parentsVisible }}
						action={(e) => (toggleNodeVisibilityLayerPanel(listing.entry.id), e?.stopPropagation())}
						size={24}
						icon={listing.entry.visible ? "EyeVisible" : "EyeHidden"}
						hoverIcon={listing.entry.visible ? "EyeHide" : "EyeShow"}
						tooltip={listing.entry.visible ? "Hide" : "Show"}
					/>
				</LayoutRow>
			{/each}
		</LayoutCol>
		{#if draggingData && !draggingData.highlightFolder && dragInPanel}
			<div class="insert-mark" style:left={`${4 + draggingData.insertDepth * 16}px`} style:top={`${draggingData.markerHeight}px`} />
		{/if}
	</LayoutRow>
</LayoutCol>

<style lang="scss" global>
	.layers {
		// Options bar
		.options-bar {
			height: 32px;
			flex: 0 0 auto;
			margin: 0 4px;

			.widget-span {
				width: 100%;
				height: 100%;
				min-width: 300px;
			}

			// Blend mode selector and opacity slider
			.dropdown-input,
			.number-input {
				flex: 1 1 auto;
			}

			// Blend mode selector
			.dropdown-input {
				max-width: 120px;
				flex-basis: 120px;
			}

			// Opacity slider
			.number-input {
				max-width: 180px;
				flex-basis: 180px;

				+ .separator ~ .separator {
					flex-grow: 1;
				}
			}
		}

		// Layer hierarchy
		.list-area {
			margin: 4px 0;
			position: relative;

			.layer {
				flex: 0 0 auto;
				align-items: center;
				position: relative;
				border-bottom: 1px solid var(--color-2-mildblack);
				border-radius: 2px;
				height: 32px;
				margin: 0 4px;
				padding-left: calc(var(--layer-indent-levels) * 16px);

				// Dimming
				&.selected {
					// Halfway between 3-darkgray and 4-dimgray (this interpolation approach only works on grayscale values)
					--component: calc((Max(var(--color-3-darkgray-rgb)) + Max(var(--color-4-dimgray-rgb))) / 2);
					background: rgb(var(--component), var(--component), var(--component));

					&.full-highlight {
						background: var(--color-4-dimgray);
					}
				}

				&.insert-folder {
					outline: 3px solid var(--color-e-nearwhite);
					outline-offset: -3px;
				}

				.expand-arrow {
					padding: 0;
					margin: 0;
					width: 16px;
					height: 100%;
					border: none;
					position: relative;
					background: none;
					flex: 0 0 auto;
					display: flex;
					align-items: center;
					justify-content: center;
					border-radius: 2px;

					&::after {
						content: "";
						position: absolute;
						width: 8px;
						height: 8px;
						background: var(--icon-expand-collapse-arrow);
					}

					&[disabled]::after {
						background: var(--icon-expand-collapse-arrow-disabled);
					}

					&:hover:not([disabled]) {
						background: var(--color-5-dullgray);

						&::after {
							background: var(--icon-expand-collapse-arrow-hover);
						}
					}

					&.expanded::after {
						transform: rotate(90deg);
					}
				}

				.thumbnail {
					width: 36px;
					height: 24px;
					margin-left: 4px;
					border-radius: 2px;
					flex: 0 0 auto;
					background-image: var(--color-transparent-checkered-background);
					background-size: var(--color-transparent-checkered-background-size-mini);
					background-position: var(--color-transparent-checkered-background-position-mini);
					background-repeat: var(--color-transparent-checkered-background-repeat);

					&:first-child {
						margin-left: 20px;
					}

					svg {
						width: calc(100% - 4px);
						height: calc(100% - 4px);
						margin: 2px;
					}
				}

				.layer-type-icon {
					flex: 0 0 auto;
					margin-left: 8px;
					margin-right: -4px;
				}

				.layer-name {
					flex: 1 1 100%;
					margin: 0 8px;

					input {
						color: inherit;
						background: none;
						border: none;
						outline: none; // Ok for input element
						margin: 0;
						padding: 0;
						text-overflow: ellipsis;
						white-space: nowrap;
						overflow: hidden;
						border-radius: 2px;
						height: 24px;
						width: 100%;

						&:disabled {
							-webkit-user-select: none; // Required as of Safari 15.0 (Graphite's minimum version) through the latest release
							user-select: none;
							// Workaround for `user-select: none` not working on <input> elements
							pointer-events: none;
						}

						&:focus {
							background: var(--color-1-nearblack);
							padding: 0 4px;

							&::placeholder {
								opacity: 0.5;
							}
						}

						&::placeholder {
							opacity: 1;
							color: inherit;
						}
					}
				}

				.status-toggle {
					flex: 0 0 auto;
					align-items: center;
					height: 100%;

					&.inactive {
						background-image: var(--background-inactive-stripes);
					}

					.icon-button {
						height: 100%;
						width: calc(24px + 2 * 4px);
					}
				}
			}

			.insert-mark {
				position: absolute;
				left: 4px;
				right: 4px;
				background: var(--color-e-nearwhite);
				margin-top: -3px;
				height: 5px;
				z-index: 1;
				pointer-events: none;
			}
		}
	}
</style>

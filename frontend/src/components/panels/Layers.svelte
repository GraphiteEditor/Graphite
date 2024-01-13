<script lang="ts">
	import { getContext, onMount, tick } from "svelte";

	import { beginDraggingElement } from "@graphite/io-managers/drag";
	import type { NodeGraphState } from "@graphite/state-providers/node-graph";
	import { platformIsMac } from "@graphite/utility-functions/platform";
	import type { Editor } from "@graphite/wasm-communication/editor";
	import { defaultWidgetLayout, patchWidgetLayout, UpdateDocumentLayerDetails, UpdateDocumentLayerStructureJs, UpdateLayersPanelOptionsLayout } from "@graphite/wasm-communication/messages";
	import type { DataBuffer, LayerClassification, LayerPanelEntry } from "@graphite/wasm-communication/messages";

	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import IconButton from "@graphite/components/widgets/buttons/IconButton.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";
	import WidgetLayout from "@graphite/components/widgets/WidgetLayout.svelte";

	type LayerListingInfo = {
		folderIndex: number;
		bottomLayer: boolean;
		editingName: boolean;
		entry: LayerPanelEntry;
	};

	const RANGE_TO_INSERT_WITHIN_BOTTOM_FOLDER_NOT_ROOT = 20;
	const INSERT_MARK_OFFSET = 2;

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

	let list: LayoutCol | undefined;

	// Layer data
	let layerCache = new Map<string, LayerPanelEntry>(); // TODO: replace with BigUint64Array as index
	let layers: LayerListingInfo[] = [];

	// Interactive dragging
	let draggable = true;
	let draggingData: undefined | DraggingData = undefined;
	let fakeHighlight: undefined | bigint = undefined;
	let dragInPanel = false;

	// Layouts
	let layersPanelOptionsLayout = defaultWidgetLayout();

	onMount(() => {
		editor.subscriptions.subscribeJsMessage(UpdateLayersPanelOptionsLayout, (updateLayersPanelOptionsLayout) => {
			patchWidgetLayout(layersPanelOptionsLayout, updateLayersPanelOptionsLayout);
			layersPanelOptionsLayout = layersPanelOptionsLayout;
		});

		editor.subscriptions.subscribeJsMessage(UpdateDocumentLayerStructureJs, (updateDocumentLayerStructure) => {
			const structure = newUpdateDocumentLayerStructure(updateDocumentLayerStructure.dataBuffer);
			rebuildLayerHierarchy(structure);
		});

		editor.subscriptions.subscribeJsMessage(UpdateDocumentLayerDetails, (updateDocumentLayerDetails) => {
			const targetLayer = updateDocumentLayerDetails.data;
			const targetId = targetLayer.id;

			updateLayerInTree(targetId, targetLayer);
		});
	});

	type DocumentLayerStructure = {
		layerId: bigint;
		children: DocumentLayerStructure[];
	};

	function newUpdateDocumentLayerStructure(dataBuffer: DataBuffer): DocumentLayerStructure {
		const pointerNum = Number(dataBuffer.pointer);
		const lengthNum = Number(dataBuffer.length);

		const wasmMemoryBuffer = editor.raw.buffer;

		// Decode the folder structure encoding
		const encoding = new DataView(wasmMemoryBuffer, pointerNum, lengthNum);

		// The structure section indicates how to read through the upcoming layer list and assign depths to each layer
		const structureSectionLength = Number(encoding.getBigUint64(0, true));
		const structureSectionMsbSigned = new DataView(wasmMemoryBuffer, pointerNum + 8, structureSectionLength * 8);

		// The layer IDs section lists each layer ID sequentially in the tree, as it will show up in the panel
		const layerIdsSection = new DataView(wasmMemoryBuffer, pointerNum + 8 + structureSectionLength * 8);

		let layersEncountered = 0;
		let currentFolder: DocumentLayerStructure = { layerId: BigInt(-1), children: [] };
		const currentFolderStack = [currentFolder];

		for (let i = 0; i < structureSectionLength; i += 1) {
			const msbSigned = structureSectionMsbSigned.getBigUint64(i * 8, true);
			const msbMask = BigInt(1) << BigInt(64 - 1);

			// Set the MSB to 0 to clear the sign and then read the number as usual
			const numberOfLayersAtThisDepth = msbSigned & ~msbMask;

			// Store child folders in the current folder (until we are interrupted by an indent)
			for (let j = 0; j < numberOfLayersAtThisDepth; j += 1) {
				const layerId = layerIdsSection.getBigUint64(layersEncountered * 8, true);
				layersEncountered += 1;

				const childLayer: DocumentLayerStructure = { layerId, children: [] };
				currentFolder.children.push(childLayer);
			}

			// Check the sign of the MSB, where a 1 is a negative (outward) indent
			const subsequentDirectionOfDepthChange = (msbSigned & msbMask) === BigInt(0);
			// Inward
			if (subsequentDirectionOfDepthChange) {
				currentFolderStack.push(currentFolder);
				currentFolder = currentFolder.children[currentFolder.children.length - 1];
			}
			// Outward
			else {
				const popped = currentFolderStack.pop();
				if (!popped) throw Error("Too many negative indents in the folder structure");
				if (popped) currentFolder = popped;
			}
		}

		return currentFolder;
	}

	function toggleLayerVisibility(id: bigint) {
		editor.instance.toggleLayerVisibility(id);
	}

	function handleExpandArrowClick(id: bigint) {
		editor.instance.toggleLayerExpansion(id);
	}

	async function onEditLayerName(listing: LayerListingInfo) {
		if (listing.editingName) return;

		draggable = false;
		listing.editingName = true;
		layers = layers;

		await tick();

		const query = list?.div()?.querySelector("[data-text-input]:not([disabled])");
		const textInput = (query instanceof HTMLInputElement && query) || undefined;
		textInput?.select();
	}

	function onEditLayerNameChange(listing: LayerListingInfo, e: Event) {
		// Eliminate duplicate events
		if (!listing.editingName) return;

		draggable = true;
		listing.editingName = false;
		layers = layers;

		const name = (e.target instanceof HTMLInputElement && e.target.value) || "";
		editor.instance.setLayerName(listing.entry.id, name);
		listing.entry.name = name;
	}

	async function onEditLayerNameDeselect(listing: LayerListingInfo) {
		draggable = true;
		listing.editingName = false;
		layers = layers;

		// Set it back to the original name if the user didn't enter a new name
		if (document.activeElement instanceof HTMLInputElement) document.activeElement.value = listing.entry.name;

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

		editor.instance.selectLayer(listing.entry.id, accel, shift);
	}

	async function deselectAllLayers() {
		editor.instance.deselectAllLayers();
	}

	function isNestingLayer(layerClassification: LayerClassification) {
		return layerClassification === "Folder" || layerClassification === "Artboard";
	}

	function calculateDragIndex(tree: LayoutCol, clientY: number, select?: () => void): DraggingData {
		const treeChildren = tree.div()?.children;
		const treeOffset = tree.div()?.getBoundingClientRect().top;

		// Closest distance to the middle of the row along the Y axis
		let closest = Infinity;

		// Folder to insert into
		let insertParentId: bigint | undefined = undefined;
		let insertDepth = 0;

		// Insert index (starts at the end, essentially infinity)
		let insertIndex = undefined;

		// Whether you are inserting into a folder and should show the folder outline
		let highlightFolder = false;

		let markerHeight = 0;
		let previousHeight: number | undefined = undefined;

		if (treeChildren !== undefined && treeOffset !== undefined) {
			Array.from(treeChildren).forEach((treeChild, index) => {
				const indexAttribute = treeChild.getAttribute("data-index");
				if (!indexAttribute) return;
				const { folderIndex, entry: layer } = layers[parseInt(indexAttribute, 10)];

				const rect = treeChild.getBoundingClientRect();
				const position = rect.top + rect.height / 2;
				const distance = position - clientY;

				// Inserting above current row
				if (distance > 0 && distance < closest) {
					insertParentId = layer.parentId;
					insertDepth = layer.depth - 1;
					insertIndex = folderIndex;
					highlightFolder = false;
					closest = distance;
					markerHeight = previousHeight || treeOffset + INSERT_MARK_OFFSET;
				}
				// Inserting below current row
				else if (distance > -closest && distance > -RANGE_TO_INSERT_WITHIN_BOTTOM_FOLDER_NOT_ROOT && distance < 0) {
					if (isNestingLayer(layer.layerClassification)) {
						insertParentId = layer.id;
						insertDepth = layer.depth;
						insertIndex = 0;
						highlightFolder = true;
					} else {
						insertParentId = layer.parentId;
						insertDepth = layer.depth - 1;
						insertIndex = folderIndex + 1;
						highlightFolder = false;
					}

					closest = -distance;
					markerHeight = index === treeChildren.length - 1 ? rect.bottom - INSERT_MARK_OFFSET : rect.bottom;
				}
				// Inserting with no nesting at the end of the panel
				else if (closest === Infinity) {
					if (layer.parentId === undefined) insertIndex = folderIndex + 1;

					markerHeight = rect.bottom - INSERT_MARK_OFFSET;
				}
				previousHeight = rect.bottom;
			});
		}

		markerHeight -= treeOffset || 0;

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
	}

	function updateInsertLine(event: DragEvent) {
		// Stop the drag from being shown as cancelled
		event.preventDefault();
		dragInPanel = true;

		if (list) draggingData = calculateDragIndex(list, event.clientY, draggingData?.select);
	}

	async function drop() {
		if (draggingData && dragInPanel) {
			const { select, insertParentId, insertIndex } = draggingData;

			select?.();
			editor.instance.moveLayerInTree(insertParentId, insertIndex);
		}
		draggingData = undefined;
		fakeHighlight = undefined;
		dragInPanel = false;
	}

	function rebuildLayerHierarchy(updateDocumentLayerStructure: DocumentLayerStructure) {
		const layerWithNameBeingEdited = layers.find((layer: LayerListingInfo) => layer.editingName);
		const layerIdWithNameBeingEdited = layerWithNameBeingEdited?.entry.id;

		// Clear the layer hierarchy before rebuilding it
		layers = [];

		// Build the new layer hierarchy
		const recurse = (folder: DocumentLayerStructure) => {
			folder.children.forEach((item, index) => {
				const mapping = layerCache.get(String(item.layerId));
				if (mapping) {
					mapping.id = item.layerId;
					layers.push({
						folderIndex: index,
						bottomLayer: index === folder.children.length - 1,
						entry: mapping,
						editingName: layerIdWithNameBeingEdited === item.layerId,
					});
				}

				// Call self recursively if there are any children
				if (item.children.length >= 1) recurse(item);
			});
		};
		recurse(updateDocumentLayerStructure);
		layers = layers;
	}

	function updateLayerInTree(targetId: bigint, targetLayer: LayerPanelEntry) {
		layerCache.set(String(targetId), targetLayer);

		const layer = layers.find((layer: LayerListingInfo) => layer.entry.id === targetId);
		if (layer) {
			layer.entry = targetLayer;
			layers = layers;
		}
	}
</script>

<LayoutCol class="layers" on:dragleave={() => (dragInPanel = false)}>
	<LayoutRow class="options-bar" scrollableX={true}>
		<WidgetLayout layout={layersPanelOptionsLayout} />
	</LayoutRow>
	<LayoutRow class="list-area" scrollableY={true}>
		<LayoutCol class="list" bind:this={list} on:click={() => deselectAllLayers()} on:dragover={(e) => draggable && updateInsertLine(e)} on:dragend={() => draggable && drop()}>
			{#each layers as listing, index (String(listing.entry.id))}
				<LayoutRow
					class="layer"
					classes={{
						selected: fakeHighlight !== undefined ? fakeHighlight === listing.entry.id : $nodeGraph.selected.includes(listing.entry.id),
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
					{#if isNestingLayer(listing.entry.layerClassification)}
						<button class="expand-arrow" class:expanded={listing.entry.expanded} on:click|stopPropagation={() => handleExpandArrowClick(listing.entry.id)} tabindex="0" />
						{#if listing.entry.layerClassification === "Artboard"}
							<IconLabel icon="Artboard" class={"layer-type-icon"} />
						{:else if listing.entry.layerClassification === "Folder"}
							<IconLabel icon="Folder" class={"layer-type-icon"} />
						{/if}
					{:else}
						<div class="thumbnail">
							{#if $nodeGraph.thumbnails.has(listing.entry.id)}
								{@html $nodeGraph.thumbnails.get(listing.entry.id)}
							{/if}
						</div>
					{/if}
					<LayoutRow class="layer-name" on:dblclick={() => onEditLayerName(listing)}>
						<input
							data-text-input
							type="text"
							value={listing.entry.name}
							placeholder={listing.entry.layerClassification}
							disabled={!listing.editingName}
							on:blur={() => onEditLayerNameDeselect(listing)}
							on:keydown={(e) => e.key === "Escape" && onEditLayerNameDeselect(listing)}
							on:keydown={(e) => e.key === "Enter" && onEditLayerNameChange(listing, e)}
							on:change={(e) => onEditLayerNameChange(listing, e)}
						/>
					</LayoutRow>
					<IconButton
						class={"visibility"}
						action={(e) => (toggleLayerVisibility(listing.entry.id), e?.stopPropagation())}
						size={24}
						icon={listing.entry.disabled ? "EyeHidden" : "EyeVisible"}
						tooltip={listing.entry.disabled ? "Disabled" : "Enabled"}
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

			// Blend mode selector
			.dropdown-input {
				max-width: 120px;
			}

			// Blend mode selector and opacity slider
			.dropdown-input,
			.number-input {
				flex: 1 1 auto;
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
				height: 32px;
				margin: 0 4px;
				padding-left: calc(4px + var(--layer-indent-levels) * 16px);
				border-bottom: 1px solid var(--color-2-mildblack);
				border-radius: 2px;

				&.selected {
					background: var(--color-4-dimgray);
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

					&:hover {
						background: var(--color-5-dullgray);
					}

					&::after {
						content: "";
						position: absolute;
						width: 0;
						height: 0;
						border-style: solid;
						border-width: 3px 0 3px 6px;
						border-color: transparent transparent transparent var(--color-e-nearwhite);
					}

					&.expanded::after {
						border-width: 6px 3px 0 3px;
						border-color: var(--color-e-nearwhite) transparent transparent transparent;
					}
				}

				.layer-type-icon {
					flex: 0 0 auto;
					margin-left: 4px;
				}

				.thumbnail {
					width: 36px;
					height: 24px;
					background: white;
					border-radius: 2px;
					flex: 0 0 auto;

					svg {
						width: calc(100% - 4px);
						height: calc(100% - 4px);
						margin: 2px;
					}
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

				.visibility {
					flex: 0 0 auto;
					align-items: center;
					height: 100%;

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

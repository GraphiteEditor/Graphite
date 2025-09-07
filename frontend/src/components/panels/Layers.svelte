<script lang="ts">
	import { getContext, onMount, onDestroy, tick } from "svelte";

	import type { Editor } from "@graphite/editor";
	import { beginDraggingElement } from "@graphite/io-managers/drag";
	import {
		defaultWidgetLayout,
		patchWidgetLayout,
		UpdateDocumentLayerDetails,
		UpdateDocumentLayerStructureJs,
		UpdateLayersPanelControlBarLeftLayout,
		UpdateLayersPanelControlBarRightLayout,
		UpdateLayersPanelBottomBarLayout,
	} from "@graphite/messages";
	import type { DataBuffer, LayerPanelEntry } from "@graphite/messages";
	import type { NodeGraphState } from "@graphite/state-providers/node-graph";
	import { platformIsMac } from "@graphite/utility-functions/platform";
	import { extractPixelData } from "@graphite/utility-functions/rasterization";

	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import IconButton from "@graphite/components/widgets/buttons/IconButton.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";
	import Separator from "@graphite/components/widgets/labels/Separator.svelte";
	import WidgetLayout from "@graphite/components/widgets/WidgetLayout.svelte";

	type LayerListingInfo = {
		folderIndex: number;
		bottomLayer: boolean;
		editingName: boolean;
		entry: LayerPanelEntry;
	};

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
	let fakeHighlightOfNotYetSelectedLayerBeingDragged: undefined | bigint = undefined;
	let dragInPanel = false;

	// Interactive clipping
	let layerToClipUponClick: LayerListingInfo | undefined = undefined;
	let layerToClipAltKeyPressed = false;

	// Layouts
	let layersPanelControlBarLeftLayout = defaultWidgetLayout();
	let layersPanelControlBarRightLayout = defaultWidgetLayout();
	let layersPanelBottomBarLayout = defaultWidgetLayout();

	onMount(() => {
		editor.subscriptions.subscribeJsMessage(UpdateLayersPanelControlBarLeftLayout, (updateLayersPanelControlBarLeftLayout) => {
			patchWidgetLayout(layersPanelControlBarLeftLayout, updateLayersPanelControlBarLeftLayout);
			layersPanelControlBarLeftLayout = layersPanelControlBarLeftLayout;
		});

		editor.subscriptions.subscribeJsMessage(UpdateLayersPanelControlBarRightLayout, (updateLayersPanelControlBarRightLayout) => {
			patchWidgetLayout(layersPanelControlBarRightLayout, updateLayersPanelControlBarRightLayout);
			layersPanelControlBarRightLayout = layersPanelControlBarRightLayout;
		});

		editor.subscriptions.subscribeJsMessage(UpdateLayersPanelBottomBarLayout, (updateLayersPanelBottomBarLayout) => {
			patchWidgetLayout(layersPanelBottomBarLayout, updateLayersPanelBottomBarLayout);
			layersPanelBottomBarLayout = layersPanelBottomBarLayout;
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

		addEventListener("pointermove", clippingHover);
		addEventListener("keydown", clippingKeyPress);
		addEventListener("keyup", clippingKeyPress);
	});

	onDestroy(() => {
		editor.subscriptions.unsubscribeJsMessage(UpdateLayersPanelControlBarLeftLayout);
		editor.subscriptions.unsubscribeJsMessage(UpdateLayersPanelControlBarRightLayout);
		editor.subscriptions.unsubscribeJsMessage(UpdateLayersPanelBottomBarLayout);
		editor.subscriptions.unsubscribeJsMessage(UpdateDocumentLayerStructureJs);
		editor.subscriptions.unsubscribeJsMessage(UpdateDocumentLayerDetails);

		removeEventListener("pointermove", clippingHover);
		removeEventListener("keydown", clippingKeyPress);
		removeEventListener("keyup", clippingKeyPress);
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

	function toggleNodeVisibilityLayerPanel(id: bigint) {
		editor.handle.toggleNodeVisibilityLayerPanel(id);
	}

	function toggleLayerLock(id: bigint) {
		editor.handle.toggleLayerLock(id);
	}

	function handleExpandArrowClickWithModifiers(e: MouseEvent, id: bigint) {
		const accel = platformIsMac() ? e.metaKey : e.ctrlKey;
		const collapseRecursive = e.altKey || accel;
		editor.handle.toggleLayerExpansion(id, collapseRecursive);
		e.stopPropagation();
	}

	async function onEditLayerName(listing: LayerListingInfo) {
		if (listing.editingName) return;

		draggable = false;
		listing.editingName = true;
		layers = layers;

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
		layers = layers;

		const name = (e.target instanceof HTMLInputElement && e.target.value) || "";
		editor.handle.setLayerName(listing.entry.id, name);
		listing.entry.alias = name;
	}

	async function onEditLayerNameDeselect(listing: LayerListingInfo) {
		draggable = true;
		listing.editingName = false;
		layers = layers;

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

		// Alt-clicking to make a clipping mask
		if (layerToClipAltKeyPressed && layerToClipUponClick && layerToClipUponClick.entry.clippable) clipLayer(layerToClipUponClick);
		// Select the layer only if the accel and/or shift keys are pressed
		else if (!oppositeAccel && !alt) selectLayer(listing, accel, shift);

		e.stopPropagation();
	}

	function clipLayer(listing: LayerListingInfo) {
		editor.handle.clipLayer(listing.entry.id);
	}

	function clippingKeyPress(e: KeyboardEvent) {
		layerToClipAltKeyPressed = e.altKey;
	}

	function clippingHover(e: PointerEvent) {
		// Don't do anything if the user is dragging to rearrange layers
		if (dragInPanel) return;

		// Get the layer below the cursor
		const target = (e.target instanceof HTMLElement && e.target.closest("[data-layer]")) || undefined;
		if (!target) {
			layerToClipUponClick = undefined;
			return;
		}

		// Check if the cursor is near the border btween two layers
		const DISTANCE = 6;
		const distanceFromTop = e.clientY - target.getBoundingClientRect().top;
		const distanceFromBottom = target.getBoundingClientRect().bottom - e.clientY;

		const nearTop = distanceFromTop < DISTANCE;
		const nearBottom = distanceFromBottom < DISTANCE;

		// If we are not near the border, we don't want to clip
		if (!nearTop && !nearBottom) {
			layerToClipUponClick = undefined;
			return;
		}

		// If we are near the border, we want to clip the layer above the border
		const indexAttribute = target?.getAttribute("data-index") ?? undefined;
		const index = indexAttribute ? Number(indexAttribute) : undefined;
		const layer = index !== undefined && layers[nearTop ? index - 1 : index];
		if (!layer) return;

		// Update the state used to show the clipping action
		layerToClipUponClick = layer;
		layerToClipAltKeyPressed = e.altKey;
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
				const { folderIndex, entry: layer } = layers[parseInt(indexAttribute, 10)];

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
				const numberRootLayers = layers.filter((layer) => layer.entry.depth === 1).length;
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
			fakeHighlightOfNotYetSelectedLayerBeingDragged = layer.id;
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
		if (!draggable) return;

		// Stop the drag from being shown as cancelled
		event.preventDefault();
		dragInPanel = true;

		if (list) draggingData = calculateDragIndex(list, event.clientY, draggingData?.select);
	}

	function drop(e: DragEvent) {
		if (!draggingData) return;
		const { select, insertParentId, insertIndex } = draggingData;

		e.preventDefault();

		if (e.dataTransfer) {
			// Moving layers
			if (e.dataTransfer.items.length === 0) {
				if (draggable && dragInPanel) {
					select?.();
					editor.handle.moveLayerInTree(insertParentId, insertIndex);
				}
			}
			// Importing files
			else {
				Array.from(e.dataTransfer.items).forEach(async (item) => {
					const file = item.getAsFile();
					if (!file) return;

					if (file.type.includes("svg")) {
						const svgData = await file.text();
						editor.handle.pasteSvg(file.name, svgData, undefined, undefined, insertParentId, insertIndex);
						return;
					}

					if (file.type.startsWith("image")) {
						const imageData = await extractPixelData(file);
						editor.handle.pasteImage(file.name, new Uint8Array(imageData.data), imageData.width, imageData.height, undefined, undefined, insertParentId, insertIndex);
						return;
					}

					// When we eventually have sub-documents, this should be changed to import the document instead of opening it in a separate tab
					const graphiteFileSuffix = "." + editor.handle.fileExtension();
					if (file.name.endsWith(graphiteFileSuffix)) {
						const content = await file.text();
						const documentName = file.name.slice(0, -graphiteFileSuffix.length);
						editor.handle.openDocumentFile(documentName, content);
						return;
					}
				});
			}
		}

		draggingData = undefined;
		fakeHighlightOfNotYetSelectedLayerBeingDragged = undefined;
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
	<LayoutRow class="control-bar" scrollableX={true}>
		<WidgetLayout layout={layersPanelControlBarLeftLayout} />
		{#if layersPanelControlBarLeftLayout?.layout?.length > 0 && layersPanelControlBarRightLayout?.layout?.length > 0}
			<Separator />
		{/if}
		<WidgetLayout layout={layersPanelControlBarRightLayout} />
	</LayoutRow>
	<LayoutRow class="list-area" scrollableY={true}>
		<LayoutCol
			class="list"
			styles={{ cursor: layerToClipUponClick && layerToClipAltKeyPressed && layerToClipUponClick.entry.clippable ? "alias" : "auto" }}
			data-layer-panel
			bind:this={list}
			on:click={() => deselectAllLayers()}
			on:dragover={updateInsertLine}
			on:dragend={drop}
			on:drop={drop}
		>
			{#each layers as listing, index}
				{@const selected = fakeHighlightOfNotYetSelectedLayerBeingDragged !== undefined ? fakeHighlightOfNotYetSelectedLayerBeingDragged === listing.entry.id : listing.entry.selected}
				<LayoutRow
					class="layer"
					classes={{
						selected,
						"ancestor-of-selected": listing.entry.ancestorOfSelected,
						"descendant-of-selected": listing.entry.descendantOfSelected,
						"selected-but-not-in-selected-network": selected && !listing.entry.inSelectedNetwork,
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
							title={listing.entry.expanded
								? "Collapse (Click) / Collapse All (Alt Click)"
								: `Expand (Click) / Expand All (Alt Click)${listing.entry.ancestorOfSelected ? "\n(A selected layer is contained within)" : ""}`}
							on:click={(e) => handleExpandArrowClickWithModifiers(e, listing.entry.id)}
							tabindex="0"
						></button>
					{:else}
						<div class="expand-arrow-none"></div>
					{/if}
					{#if listing.entry.clipped}
						<IconLabel icon="Clipped" class="clipped-arrow" tooltip="Clipping mask is active (Alt-click border to release)" />
					{/if}
					<div class="thumbnail">
						{#if $nodeGraph.thumbnails.has(listing.entry.id)}
							{@html $nodeGraph.thumbnails.get(listing.entry.id)}
						{/if}
					</div>
					{#if listing.entry.name === "Artboard"}
						<IconLabel icon="Artboard" class="layer-type-icon" />
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
							class="status-toggle"
							classes={{ inherited: !listing.entry.parentsUnlocked }}
							action={(e) => (toggleLayerLock(listing.entry.id), e?.stopPropagation())}
							size={24}
							icon={listing.entry.unlocked ? "PadlockUnlocked" : "PadlockLocked"}
							hoverIcon={listing.entry.unlocked ? "PadlockLocked" : "PadlockUnlocked"}
							tooltip={(listing.entry.unlocked ? "Lock" : "Unlock") + (!listing.entry.parentsUnlocked ? "\n(A parent of this layer is locked and that status is being inherited)" : "")}
						/>
					{/if}
					<IconButton
						class="status-toggle"
						classes={{ inherited: !listing.entry.parentsVisible }}
						action={(e) => (toggleNodeVisibilityLayerPanel(listing.entry.id), e?.stopPropagation())}
						size={24}
						icon={listing.entry.visible ? "EyeVisible" : "EyeHidden"}
						hoverIcon={listing.entry.visible ? "EyeHide" : "EyeShow"}
						tooltip={(listing.entry.visible ? "Hide" : "Show") + (!listing.entry.parentsVisible ? "\n(A parent of this layer is hidden and that status is being inherited)" : "")}
					/>
				</LayoutRow>
			{/each}
		</LayoutCol>
		{#if draggingData && !draggingData.highlightFolder && dragInPanel}
			<div class="insert-mark" style:left={`${4 + draggingData.insertDepth * 16}px`} style:top={`${draggingData.markerHeight}px`} />
		{/if}
	</LayoutRow>
	<LayoutRow class="bottom-bar" scrollableX={true}>
		<WidgetLayout layout={layersPanelBottomBarLayout} />
	</LayoutRow>
</LayoutCol>

<style lang="scss" global>
	.layers {
		// Control bar
		.control-bar {
			height: 32px;
			flex: 0 0 auto;
			margin: 0 4px;
			border-bottom: 1px solid var(--color-2-mildblack);
			justify-content: space-between;

			.widget-span:first-child {
				flex: 1 1 auto;
			}

			&:not(:has(*)) {
				display: none;
			}
		}

		// Bottom bar
		.bottom-bar {
			height: 24px;
			padding-top: 4px;
			flex: 0 0 auto;
			margin: 0 4px;
			justify-content: flex-end;
			border-top: 1px solid var(--color-2-mildblack);

			.widget-span > * {
				margin: 0;
			}

			&:not(:has(*)) {
				display: none;
			}
		}

		// Layer hierarchy
		.list-area {
			position: relative;
			margin-top: 4px;
			// Combine with the bottom bar to avoid a double border
			margin-bottom: -1px;

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
					background: var(--color-4-dimgray);
				}

				&.ancestor-of-selected .expand-arrow:not(.expanded) {
					background-image: var(--inheritance-dots-background-6-lowergray);
				}

				&.descendant-of-selected {
					background-image: var(--inheritance-dots-background-4-dimgray);
				}

				&.selected-but-not-in-selected-network {
					background: rgba(var(--color-4-dimgray-rgb), 0.5);
				}

				&.insert-folder {
					outline: 3px solid var(--color-e-nearwhite);
					outline-offset: -3px;
				}

				.expand-arrow {
					padding: 0;
					margin: 0;
					margin-right: 4px;
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

				.expand-arrow-none {
					flex: 0 0 16px;
					margin-right: 4px;
				}

				.clipped-arrow {
					margin-left: 2px;
					margin-right: 2px;
				}

				.thumbnail {
					width: 36px;
					height: 24px;
					border-radius: 2px;
					overflow: hidden;
					flex: 0 0 auto;
					background-image: var(--color-transparent-checkered-background);
					background-size: var(--color-transparent-checkered-background-size-mini);
					background-position: var(--color-transparent-checkered-background-position-mini);
					background-repeat: var(--color-transparent-checkered-background-repeat);

					svg {
						width: 100%;
						height: 100%;
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
							-webkit-user-select: none; // Still required by Safari as of 2025
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

					&.inherited {
						background-image: var(--inheritance-stripes-background);
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

<script lang="ts">
	import { getContext, onMount, onDestroy, tick } from "svelte";
	import LayoutCol from "/src/components/layout/LayoutCol.svelte";
	import LayoutRow from "/src/components/layout/LayoutRow.svelte";
	import IconButton from "/src/components/widgets/buttons/IconButton.svelte";
	import IconLabel from "/src/components/widgets/labels/IconLabel.svelte";
	import Separator from "/src/components/widgets/labels/Separator.svelte";
	import WidgetLayout from "/src/components/widgets/WidgetLayout.svelte";
	import type { NodeGraphStore } from "/src/stores/node-graph";
	import type { PortfolioStore } from "/src/stores/portfolio";
	import type { TooltipStore } from "/src/stores/tooltip";
	import { pasteFile } from "/src/utility-functions/files";
	import { operatingSystem } from "/src/utility-functions/platform";
	import type { EditorWrapper, LayerPanelEntry, LayerStructureEntry } from "/wrapper/pkg/graphite_wasm_wrapper";

	type LayerListingInfo = {
		folderIndex: number;
		bottomLayer: boolean;
		editingName: boolean;
		entry: LayerPanelEntry;
		depth: number;
		parentId: bigint | undefined;
		childrenPresent: boolean;
		expanded: boolean;
		ancestorOfSelected: boolean;
		parentsVisible: boolean;
		parentsUnlocked: boolean;
		treePath: bigint[];
	};

	type DraggingData = {
		select?: () => void;
		insertParentId: bigint | undefined;
		insertDepth: number;
		insertIndex: number | undefined;
		highlightFolder: boolean;
		highlightFolderIndex: number | undefined;
		markerHeight: number;
	};

	type InternalDragState = {
		active: boolean;
		layerId: bigint;
		listing: LayerListingInfo;
		startX: number;
		startY: number;
	};

	const editor = getContext<EditorWrapper>("editor");
	const nodeGraph = getContext<NodeGraphStore>("nodeGraph");
	const tooltip = getContext<TooltipStore>("tooltip");
	const portfolio = getContext<PortfolioStore>("portfolio");

	let list: LayoutCol | undefined;

	let layers: LayerListingInfo[] = [];

	// Interactive dragging
	let draggable = true;
	let draggingData: undefined | DraggingData = undefined;
	let internalDragState: InternalDragState | undefined = undefined;
	let fakeHighlightOfNotYetSelectedLayerBeingDragged: undefined | bigint = undefined;
	let justFinishedDrag = false; // Used to prevent click events after a drag
	let dragInPanel = false;

	// Alt+Drag duplication
	let altKeyPressedDuringDrag = false;
	let originalLayersBeforeDuplication: bigint[] | undefined = undefined;
	let duplicatedLayerIds: bigint[] | undefined = undefined;

	// Interactive clipping
	let layerToClipUponClick: LayerListingInfo | undefined = undefined;
	let layerToClipAltKeyPressed = false;

	$: rebuildLayerHierarchy($portfolio.layerStructure, $portfolio.layerCache);

	onMount(() => {
		addEventListener("pointerup", draggingPointerUp);
		addEventListener("pointermove", draggingPointerMove);
		addEventListener("mousedown", draggingMouseDown);
		addEventListener("keydown", draggingKeyDown);
		addEventListener("keydown", handleLayerPanelKeyDown);
		addEventListener("keyup", draggingKeyUp);
		addEventListener("blur", () => internalDragState?.active && abortDrag());

		addEventListener("pointermove", clippingHover);
		addEventListener("keydown", clippingKeyPress);
		addEventListener("keyup", clippingKeyPress);
	});

	onDestroy(() => {
		removeEventListener("pointerup", draggingPointerUp);
		removeEventListener("pointermove", draggingPointerMove);
		removeEventListener("mousedown", draggingMouseDown);
		removeEventListener("keydown", draggingKeyDown);
		removeEventListener("keydown", handleLayerPanelKeyDown);
		removeEventListener("keyup", draggingKeyUp);

		removeEventListener("pointermove", clippingHover);
		removeEventListener("keydown", clippingKeyPress);
		removeEventListener("keyup", clippingKeyPress);
	});

	function toggleNodeVisibilityLayerPanel(id: bigint) {
		editor.toggleNodeVisibilityLayerPanel(id);
	}

	function toggleLayerLock(id: bigint) {
		editor.toggleLayerLock(id);
	}

	function handleExpandArrowClickWithModifiers(e: MouseEvent, treePath: bigint[]) {
		const accel = operatingSystem() === "Mac" ? e.metaKey : e.ctrlKey;
		const collapseRecursive = e.altKey || accel;
		editor.toggleLayerExpansion(BigUint64Array.from(treePath), collapseRecursive);
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
		editor.setLayerName(listing.entry.id, name);
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
		if (justFinishedDrag) {
			justFinishedDrag = false;
			// Prevent bubbling to deselectAllLayers
			e.stopPropagation();
			return;
		}

		// Get the pressed state of the modifier keys
		const [ctrl, meta, shift, alt] = [e.ctrlKey, e.metaKey, e.shiftKey, e.altKey];
		// Get the state of the platform's accel key and its opposite platform's accel key
		const [accel, oppositeAccel] = operatingSystem() === "Mac" ? [meta, ctrl] : [ctrl, meta];

		// Alt-clicking to make a clipping mask
		if (layerToClipAltKeyPressed && layerToClipUponClick && layerToClipUponClick.entry.clippable) clipLayer(layerToClipUponClick);
		// Select the layer only if the accel and/or shift keys are pressed
		else if (!oppositeAccel && !alt) selectLayer(listing, accel, shift);

		e.stopPropagation();
	}

	function clipLayer(listing: LayerListingInfo) {
		editor.clipLayer(listing.entry.id);
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

		// Check if the cursor is near the border between two layers
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

		editor.selectLayer(listing.entry.id, accel, shift);
	}

	async function deselectAllLayers() {
		if (justFinishedDrag) {
			justFinishedDrag = false;
			return;
		}

		editor.deselectAllLayers();
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
		let highlightFolderIndex: number | undefined = undefined;

		let markerHeight = 0;
		const layerPanel = document.querySelector("[data-layer-panel]"); // Selects the element with the data-layer-panel attribute
		if (layerPanel !== null && treeChildren !== undefined && treeOffset !== undefined) {
			let layerPanelTop = layerPanel.getBoundingClientRect().top;
			Array.from(treeChildren).forEach((treeChild) => {
				const indexAttribute = treeChild.getAttribute("data-index");
				if (!indexAttribute) return;
				const listing = layers[parseInt(indexAttribute, 10)];

				const rect = treeChild.getBoundingClientRect();
				if (rect.top > clientY || rect.bottom < clientY) {
					return;
				}
				const pointerPercentage = (clientY - rect.top) / rect.height;
				if (listing.entry.childrenAllowed || listing.childrenPresent) {
					if (pointerPercentage < 0.25) {
						insertParentId = listing.parentId;
						insertDepth = listing.depth - 1;
						insertIndex = listing.folderIndex;
						markerHeight = rect.top - layerPanelTop;
					} else if (pointerPercentage < 0.75 || (listing.childrenPresent && listing.expanded)) {
						insertParentId = listing.entry.id;
						insertDepth = listing.depth;
						insertIndex = 0;
						highlightFolder = true;
						highlightFolderIndex = parseInt(indexAttribute, 10);
					} else {
						insertParentId = listing.parentId;
						insertDepth = listing.depth - 1;
						insertIndex = listing.folderIndex + 1;
						markerHeight = rect.bottom - layerPanelTop;
					}
				} else {
					if (pointerPercentage < 0.5) {
						insertParentId = listing.parentId;
						insertDepth = listing.depth - 1;
						insertIndex = listing.folderIndex;
						markerHeight = rect.top - layerPanelTop;
					} else {
						insertParentId = listing.parentId;
						insertDepth = listing.depth - 1;
						insertIndex = listing.folderIndex + 1;
						markerHeight = rect.bottom - layerPanelTop;
					}
				}
			});
			// Dragging to the empty space below all layers
			let lastLayer = treeChildren[treeChildren.length - 1];
			if (lastLayer.getBoundingClientRect().bottom < clientY) {
				const numberRootLayers = layers.filter((listing) => listing.depth === 1).length;
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
			highlightFolderIndex,
			markerHeight,
		};
	}

	function layerPointerDown(e: PointerEvent, listing: LayerListingInfo) {
		// Only left click drags
		if (e.button !== 0 || !draggable) return;

		internalDragState = {
			active: false,
			layerId: listing.entry.id,
			listing: listing,
			startX: e.clientX,
			startY: e.clientY,
		};
	}

	function draggingPointerMove(e: PointerEvent) {
		if (!internalDragState || !list) return;

		// Calculate distance moved
		if (!internalDragState.active) {
			const distance = Math.hypot(e.clientX - internalDragState.startX, e.clientY - internalDragState.startY);
			const DRAG_THRESHOLD = 5;

			if (distance > DRAG_THRESHOLD) {
				internalDragState.active = true;
				dragInPanel = true;

				const layer = internalDragState.listing.entry;
				if (!$nodeGraph.selected.includes(layer.id)) {
					fakeHighlightOfNotYetSelectedLayerBeingDragged = layer.id;
				}
			}
		}

		// Perform drag calculations if a drag is occurring
		if (internalDragState.active) {
			const select = () => {
				if (internalDragState && !$nodeGraph.selected.includes(internalDragState.layerId)) {
					selectLayer(internalDragState.listing, false, false);
				}
			};

			draggingData = calculateDragIndex(list, e.clientY, select);
		}
	}

	function draggingPointerUp() {
		if (internalDragState?.active && draggingData) {
			const { select, insertParentId, insertIndex } = draggingData;

			// Commit the move
			select?.();
			editor.moveLayerInTree(insertParentId, insertIndex);

			// Prevent the subsequent click event from processing
			justFinishedDrag = true;
		} else if (justFinishedDrag) {
			// Avoid right-click abort getting stuck with `justFinishedDrag` set and blocking the first subsequent click to select a layer
			setTimeout(() => {
				justFinishedDrag = false;
			}, 0);
		}

		// Reset state
		abortDrag();
	}

	async function startDuplicates() {
		if (!internalDragState?.active) return;

		originalLayersBeforeDuplication = [...$nodeGraph.selected];
		editor.duplicateSelectedLayers();

		await tick();
		duplicatedLayerIds = [...$nodeGraph.selected];
	}

	function stopDuplicates() {
		if (!originalLayersBeforeDuplication || !duplicatedLayerIds) return;

		duplicatedLayerIds.forEach((layerId) => {
			editor.deleteNode(layerId);
		});

		editor.deselectAllLayers();
		originalLayersBeforeDuplication.forEach((layerId, index) => {
			const ctrl = index > 0;
			editor.selectLayer(layerId, ctrl, false);
		});

		originalLayersBeforeDuplication = undefined;
		duplicatedLayerIds = undefined;
	}

	function abortDrag() {
		if (altKeyPressedDuringDrag && originalLayersBeforeDuplication) {
			stopDuplicates();
		}

		internalDragState = undefined;
		draggingData = undefined;
		fakeHighlightOfNotYetSelectedLayerBeingDragged = undefined;
		dragInPanel = false;
		altKeyPressedDuringDrag = false;
		originalLayersBeforeDuplication = undefined;
		duplicatedLayerIds = undefined;
	}

	function draggingMouseDown(e: MouseEvent) {
		// Abort if a drag is active and the user presses the right mouse button (button 2)
		if (e.button === 2 && internalDragState?.active) {
			justFinishedDrag = true;
			abortDrag();
		}
	}

	function draggingKeyDown(e: KeyboardEvent) {
		if (e.key === "Escape" && internalDragState?.active) {
			justFinishedDrag = true;
			abortDrag();
		}

		if (e.key === "Alt" && internalDragState?.active && !altKeyPressedDuringDrag) {
			altKeyPressedDuringDrag = true;
			startDuplicates();
		}
	}

	function draggingKeyUp(e: KeyboardEvent) {
		if (e.key === "Alt" && internalDragState?.active && altKeyPressedDuringDrag) {
			altKeyPressedDuringDrag = false;
			stopDuplicates();
		}
	}

	function handleLayerPanelKeyDown(e: KeyboardEvent) {
		// TODO: Handle this F2 shortcut detection in the backend, not frontend, so it uses the standard key binding system

		// Only handle F2 if not currently editing a layer name
		if (e.key === "F2" && !layers.some((layer) => layer.editingName)) {
			// Find the first selected layer
			const selectedLayer = layers.find((layer) => layer.entry.selected);
			if (selectedLayer) {
				e.preventDefault();
				onEditLayerName(selectedLayer);
			}
		}
	}

	async function navigateToLayer(currentListing: LayerListingInfo, direction: "Up" | "Down") {
		// Save the current layer name
		const inputElement = document.activeElement;
		if (inputElement instanceof HTMLInputElement) {
			const name = inputElement.value || "";
			editor.setLayerName(currentListing.entry.id, name);
			currentListing.entry.alias = name;
		}

		// Find current layer index
		const currentIndex = layers.findIndex((layer) => layer.entry.id === currentListing.entry.id);
		if (currentIndex === -1) return;

		// Calculate target index based on direction
		const targetIndex = direction === "Down" ? currentIndex + 1 : currentIndex - 1;
		if (targetIndex >= layers.length || targetIndex < 0) return;

		const targetListing = layers[targetIndex];
		if (!targetListing) return;

		// Exit edit mode on current layer
		currentListing.editingName = false;
		draggable = true;
		layers = layers;

		// Start edit mode on target layer
		await onEditLayerName(targetListing);
	}

	function fileDragOver(e: DragEvent) {
		if (!draggable || !e.dataTransfer || !e.dataTransfer.types.includes("Files")) return;

		// Stop the drag from being shown as cancelled
		e.preventDefault();
		dragInPanel = true;

		if (list) draggingData = calculateDragIndex(list, e.clientY);
	}

	function fileDrop(e: DragEvent) {
		if (!draggingData || !e.dataTransfer || !e.dataTransfer.types.includes("Files")) return;

		const { insertParentId, insertIndex } = draggingData;

		e.preventDefault();

		Array.from(e.dataTransfer.items).forEach(async (item) => await pasteFile(item, editor, undefined, insertParentId, insertIndex));

		draggingData = undefined;
		fakeHighlightOfNotYetSelectedLayerBeingDragged = undefined;
		dragInPanel = false;
	}

	function rebuildLayerHierarchy(layerStructure: LayerStructureEntry[], cache: Map<string, LayerPanelEntry>) {
		// Track the editing state by flat list index, not layer ID, since a layer can appear at multiple positions
		const editingIndex = layers.findIndex((layer: LayerListingInfo) => layer.editingName);

		// Clear the layer hierarchy before rebuilding it
		layers = [];

		// Build the new layer hierarchy
		const recurse = (children: LayerStructureEntry[], depth: number, parentId: bigint | undefined, parentPath: bigint[], parentsVisible: boolean, parentsUnlocked: boolean) => {
			children.forEach((item, index) => {
				const treePath = [...parentPath, item.layerId];
				const mapping = cache.get(String(item.layerId));

				if (mapping) {
					mapping.id = item.layerId;
					layers.push({
						folderIndex: index,
						bottomLayer: index === children.length - 1,
						entry: mapping,
						editingName: editingIndex === layers.length,
						depth,
						parentId,
						childrenPresent: item.childrenPresent,
						expanded: item.childrenPresent && item.children.length > 0,
						ancestorOfSelected: item.descendantSelected,
						parentsVisible,
						parentsUnlocked,
						treePath,
					});
				}

				// Call self recursively, propagating this layer's visibility/lock state to its children
				const childParentsVisible = parentsVisible && (mapping?.visible ?? true);
				const childParentsUnlocked = parentsUnlocked && (mapping?.unlocked ?? true);
				if (item.children.length >= 1) recurse(item.children, depth + 1, item.layerId, treePath, childParentsVisible, childParentsUnlocked);
			});
		};
		recurse(layerStructure, 1, undefined, [], true, true);
		layers = layers;
	}
</script>

<LayoutCol class="layers" on:dragleave={() => (dragInPanel = false)}>
	<LayoutRow class="control-bar" scrollableX={true}>
		<WidgetLayout layout={$portfolio.layersPanelControlBarLeftLayout} layoutTarget="LayersPanelControlLeftBar" />
		{#if $portfolio.layersPanelControlBarLeftLayout?.length > 0 && $portfolio.layersPanelControlBarRightLayout?.length > 0}
			<Separator />
		{/if}
		<WidgetLayout layout={$portfolio.layersPanelControlBarRightLayout} layoutTarget="LayersPanelControlRightBar" />
	</LayoutRow>
	<LayoutRow class="list-area" classes={{ "drag-ongoing": Boolean(internalDragState?.active && draggingData) }} scrollableY={true}>
		<LayoutCol
			class="list"
			styles={{ cursor: layerToClipUponClick && layerToClipAltKeyPressed && layerToClipUponClick.entry.clippable ? "alias" : "auto" }}
			data-layer-panel
			bind:this={list}
			on:click={() => deselectAllLayers()}
			on:dragover={fileDragOver}
			on:drop={fileDrop}
		>
			{#each layers as listing, index}
				{@const selected = fakeHighlightOfNotYetSelectedLayerBeingDragged !== undefined ? fakeHighlightOfNotYetSelectedLayerBeingDragged === listing.entry.id : listing.entry.selected}
				<LayoutRow
					class="layer"
					classes={{
						selected,
						"ancestor-of-selected": listing.ancestorOfSelected,
						"descendant-of-selected": listing.entry.descendantOfSelected,
						"selected-but-not-in-selected-network": selected && !listing.entry.inSelectedNetwork,
						"insert-folder": (draggingData?.highlightFolder || false) && draggingData?.highlightFolderIndex === index,
					}}
					styles={{ "--layer-indent-levels": `${listing.depth - 1}` }}
					data-layer
					data-index={index}
					on:pointerdown={(e) => layerPointerDown(e, listing)}
					on:click={(e) => selectLayerWithModifiers(e, listing)}
				>
					{#if listing.entry.childrenAllowed || listing.childrenPresent}
						<button
							class="expand-arrow"
							class:expanded={listing.expanded}
							disabled={!listing.childrenPresent}
							data-tooltip-label={listing.expanded ? "Collapse (All)" : "Expand (All)"}
							data-tooltip-description={(listing.expanded
								? "Hide this layer's children. (To recursively collapse all descendants, perform the shortcut shown.)"
								: "Show this layer's children. (To recursively expand all descendants, perform the shortcut shown.)") +
								(listing.ancestorOfSelected && !listing.expanded ? "\n\nA selected layer is currently contained within.\n" : "")}
							data-tooltip-shortcut={$tooltip.altClickShortcut?.shortcut ? JSON.stringify($tooltip.altClickShortcut.shortcut) : undefined}
							on:click={(e) => handleExpandArrowClickWithModifiers(e, listing.treePath)}
							tabindex="0"
						></button>
					{:else}
						<div class="expand-arrow-none"></div>
					{/if}
					{#if listing.entry.clipped}
						<IconLabel
							icon="Clipped"
							class="clipped-arrow"
							tooltipLabel="Layer Clipped"
							tooltipDescription="Clipping mask is active. To release it, target the bottom border of the layer and perform the shortcut shown."
							tooltipShortcut={$tooltip.altClickShortcut}
						/>
					{/if}
					<div class="thumbnail">
						{#if $nodeGraph.thumbnails.has(listing.entry.id)}
							{@html $nodeGraph.thumbnails.get(listing.entry.id)}
						{/if}
					</div>
					{#if listing.entry.iconName}
						<IconLabel icon={listing.entry.iconName} class="layer-type-icon" tooltipLabel="Artboard" />
					{/if}
					<LayoutRow class="layer-name" on:dblclick={() => onEditLayerName(listing)}>
						<input
							data-text-input
							type="text"
							value={listing.entry.alias}
							placeholder={listing.entry.implementationName}
							disabled={!listing.editingName}
							on:blur={() => onEditLayerNameDeselect(listing)}
							on:keydown={(e) => {
								if (e.key === "Escape") {
									onEditLayerNameDeselect(listing);
								} else if (e.key === "Enter") {
									onEditLayerNameChange(listing, e);
								} else if (e.key === "Tab") {
									e.preventDefault();
									navigateToLayer(listing, e.shiftKey ? "Up" : "Down");
								} else if (e.key === "ArrowUp") {
									e.preventDefault();
									navigateToLayer(listing, "Up");
								} else if (e.key === "ArrowDown") {
									e.preventDefault();
									navigateToLayer(listing, "Down");
								}
							}}
							on:change={(e) => onEditLayerNameChange(listing, e)}
						/>
					</LayoutRow>
					{#if !listing.entry.unlocked || !listing.parentsUnlocked}
						<IconButton
							class="status-toggle"
							classes={{ inherited: !listing.parentsUnlocked }}
							action={(e) => (toggleLayerLock(listing.entry.id), e?.stopPropagation())}
							size={24}
							icon={listing.entry.unlocked ? "PadlockUnlocked" : "PadlockLocked"}
							hoverIcon={listing.entry.unlocked ? "PadlockLocked" : "PadlockUnlocked"}
							tooltipLabel={listing.entry.unlocked ? "Lock" : "Unlock"}
							tooltipDescription={!listing.parentsUnlocked ? "A parent of this layer is locked and that status is being inherited." : ""}
						/>
					{/if}
					<IconButton
						class="status-toggle"
						classes={{ inherited: !listing.parentsVisible }}
						action={(e) => (toggleNodeVisibilityLayerPanel(listing.entry.id), e?.stopPropagation())}
						size={24}
						icon={listing.entry.visible ? "EyeVisible" : "EyeHidden"}
						hoverIcon={listing.entry.visible ? "EyeHide" : "EyeShow"}
						tooltipLabel={listing.entry.visible ? "Hide" : "Show"}
						tooltipDescription={!listing.parentsVisible ? "A parent of this layer is hidden and that status is being inherited." : ""}
					/>
				</LayoutRow>
			{/each}
		</LayoutCol>
		{#if draggingData && !draggingData.highlightFolder && dragInPanel}
			<div class="insert-mark" style:left={`${4 + draggingData.insertDepth * 16}px`} style:top={`${draggingData.markerHeight}px`}></div>
		{/if}
	</LayoutRow>
	<LayoutRow class="bottom-bar" scrollableX={true}>
		<WidgetLayout layout={$portfolio.layersPanelBottomBarLayout} layoutTarget="LayersPanelBottomBar" />
	</LayoutRow>
</LayoutCol>

<style lang="scss">
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
			padding-top: 4px;
			// Combine with the bottom bar to avoid a double border
			margin-bottom: -1px;

			&.drag-ongoing .layer {
				pointer-events: none;
			}

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

				&.insert-folder::after {
					content: "";
					position: absolute;
					inset: 0;
					border: 3px solid var(--color-e-nearwhite);
					border-radius: 2px;
					pointer-events: none;
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
				margin-top: 1px;
				height: 5px;
				z-index: 1;
				pointer-events: none;
			}
		}
	}
</style>

<script lang="ts">
	import { getContext, onMount, onDestroy, tick } from "svelte";
	import LayoutCol from "/src/components/layout/LayoutCol.svelte";
	import LayoutRow from "/src/components/layout/LayoutRow.svelte";
	import IconButton from "/src/components/widgets/buttons/IconButton.svelte";
	import IconLabel from "/src/components/widgets/labels/IconLabel.svelte";
	import WidgetLayout from "/src/components/widgets/WidgetLayout.svelte";
	import { createDragToggleManager, destroyDragToggleManager } from "/src/managers/drag-toggle";
	import type { NodeGraphStore } from "/src/stores/node-graph";
	import { layersPanelControlBarLeftLayout, layersPanelBottomBarLeftLayout, layersPanelBottomBarRightLayout } from "/src/stores/portfolio";
	import type { PortfolioStore } from "/src/stores/portfolio";
	import type { TooltipStore } from "/src/stores/tooltip";
	import { pasteFile } from "/src/utility-functions/files";
	import { operatingSystem } from "/src/utility-functions/platform";
	import type { EditorWrapper, LayerPanelChainNode, LayerPanelEntry, LayerStructureEntry } from "/wrapper/pkg/graphite_wasm_wrapper";

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

	type ChainDragState = {
		active: boolean;
		// The layer whose chain the nodes are being dragged from
		layerId: bigint;
		nodeIds: bigint[];
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
	let dragDropTarget: HTMLElement | undefined = undefined;

	// Interactive dragging of chain nodes within a layer's chain or into another layer's
	let chainDragState: ChainDragState | undefined = undefined;
	let chainDragActive = false;
	// The layer row (by its index in the list) under the cursor, the gap among its reorderable chain nodes counted from the left, and where its marker sits within the row
	let chainDropRowIndex: number | undefined = undefined;
	let chainInsertIndex: number | undefined = undefined;
	let chainInsertMarkerLeft: number | undefined = undefined;
	let chainDragDropTarget: HTMLElement | undefined = undefined;

	// Interactive clipping
	let layerToClipUponClick: LayerListingInfo | undefined = undefined;
	let layerToClipAltKeyPressed = false;

	// Drag-toggle: tracked here so the template can render the invisible lock placeholder during a `layer-lock` gesture
	let activeDragToggleGroup: string | undefined = undefined;

	$: rebuildLayerHierarchy($portfolio.layerStructure, $portfolio.layerCache);

	onMount(() => {
		createDragToggleManager(dragToggleListener);

		addEventListener("pointerup", draggingPointerUp);
		addEventListener("pointermove", draggingPointerMove);
		addEventListener("mousedown", draggingMouseDown);
		addEventListener("keydown", draggingKeyDown);
		addEventListener("keydown", handleLayerPanelKeyDown);

		addEventListener("pointermove", clippingHover);
		addEventListener("keydown", clippingKeyPress);
		addEventListener("keyup", clippingKeyPress);
	});

	onDestroy(() => {
		destroyDragToggleManager(dragToggleListener);

		removeEventListener("pointerup", draggingPointerUp);
		removeEventListener("pointermove", draggingPointerMove);
		removeEventListener("mousedown", draggingMouseDown);
		removeEventListener("keydown", draggingKeyDown);
		removeEventListener("keydown", handleLayerPanelKeyDown);

		removeEventListener("pointermove", clippingHover);
		removeEventListener("keydown", clippingKeyPress);
		removeEventListener("keyup", clippingKeyPress);
	});

	function dragToggleListener(group: string | undefined) {
		activeDragToggleGroup = group;
	}

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

		const query = list?.div?.()?.querySelector("[data-layer-name-input]:enabled");
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

	function chainNodeClick(e: MouseEvent | undefined, listing: LayerListingInfo, nodeId: bigint) {
		e?.stopPropagation();

		// The click that follows releasing a drag shouldn't select anything
		if (justFinishedDrag) return;

		// Same modifier key handling as selecting layers
		const [accel, oppositeAccel] = operatingSystem() === "Mac" ? [e?.metaKey, e?.ctrlKey] : [e?.ctrlKey, e?.metaKey];
		if (oppositeAccel || e?.altKey) return;

		editor.selectChainNode(listing.entry.id, nodeId, Boolean(accel), Boolean(e?.shiftKey));
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
		if (layerPanel && treeChildren && treeOffset !== undefined) {
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

		// Pressing one of the chain's node buttons drags that node within the chain, rather than dragging the whole layer
		const chainNodeButton = (e.target instanceof Element && e.target.closest("[data-chain-node]")) || undefined;
		if (chainNodeButton) {
			const chainNode = listing.entry.chainNodes[Number(chainNodeButton.getAttribute("data-chain-node"))];
			if (chainNode) chainNodePointerDown(e, listing, chainNode);
			return;
		}

		internalDragState = {
			active: false,
			layerId: listing.entry.id,
			listing: listing,
			startX: e.clientX,
			startY: e.clientY,
		};
	}

	function chainNodePointerDown(e: PointerEvent, listing: LayerListingInfo, chainNode: LayerPanelChainNode) {
		// A source node stays fixed at the chain's upstream end
		if (!chainNode.reorderable) return;

		// Dragging a selected node brings along the chain's other selected nodes, which all end up together at the drop
		const nodeIds = chainNode.selected ? listing.entry.chainNodes.filter((node) => node.reorderable && node.selected).map((node) => node.id) : [chainNode.id];

		chainDragState = { active: false, layerId: listing.entry.id, nodeIds, startX: e.clientX, startY: e.clientY };
	}

	function chainDraggingPointerMove(e: PointerEvent) {
		if (!chainDragState) return;

		if (!chainDragState.active) {
			const distance = Math.hypot(e.clientX - chainDragState.startX, e.clientY - chainDragState.startY);
			const DRAG_THRESHOLD = 5;
			if (distance <= DRAG_THRESHOLD) return;

			chainDragState.active = true;
			chainDragActive = true;
			dragInPanel = true;
			layerToClipUponClick = undefined;
		}

		chainDropRowIndex = undefined;
		chainInsertIndex = undefined;
		chainInsertMarkerLeft = undefined;

		// Over a bottom bar button accepting chain nodes, the drop goes to it instead, and elsewhere in the bottom bar it goes nowhere
		const target = e.target instanceof Element ? e.target : undefined;
		const droppable = target?.closest("[data-drag-droppable~=chain-nodes]");
		chainDragDropTarget = droppable instanceof HTMLElement ? droppable : undefined;
		if (chainDragDropTarget || target?.closest("[data-layer-bottom-bar]")) return;

		// Otherwise the nodes drop into the chain of whichever layer is under the cursor
		const row = target?.closest("[data-layer]");
		if (row) calculateChainInsertIndex(row, e.clientX);
	}

	// The row's reorderable chain node buttons, in their left-to-right order
	function reorderableChainNodeButtons(row: Element): Element[] {
		return Array.from(row.querySelectorAll("[data-chain-node-reorderable]"));
	}

	function calculateChainInsertIndex(row: Element, clientX: number) {
		const rects = reorderableChainNodeButtons(row).map((button) => button.getBoundingClientRect());

		// The insertion index is the number of reorderable nodes whose horizontal midpoint sits left of the cursor
		const firstRightOfCursor = rects.findIndex((rect) => clientX < (rect.left + rect.right) / 2);
		const index = firstRightOfCursor === -1 ? rects.length : firstRightOfCursor;

		// The marker sits on the boundary between the neighboring nodes, or at the outer edge of the node at either end.
		// With nothing reorderable, the only gap is beside the layer, after a fixed source node or else just before the layer's icon.
		let markerX: number | undefined;
		if (rects.length === 0) {
			const chainNodeButtons = row.querySelectorAll("[data-chain-node]");
			const fixedSource = chainNodeButtons[chainNodeButtons.length - 1];
			markerX = fixedSource ? fixedSource.getBoundingClientRect().right : row.querySelector("[data-layer-chain]")?.getBoundingClientRect().left;
		} else if (index === 0) {
			markerX = rects[0].left;
		} else if (index === rects.length) {
			markerX = rects[rects.length - 1].right;
		} else {
			markerX = (rects[index - 1].right + rects[index].left) / 2;
		}
		if (markerX === undefined) return;

		chainDropRowIndex = Number(row.getAttribute("data-index"));
		chainInsertIndex = index;
		chainInsertMarkerLeft = markerX - row.getBoundingClientRect().left;
	}

	// Swallow the click fired by releasing a drag, without leaving the flag set to block a later click if none comes
	function swallowReleaseClick() {
		justFinishedDrag = true;
		setTimeout(() => (justFinishedDrag = false), 0);
	}

	function chainDraggingPointerUp(e: PointerEvent) {
		if (chainDragState?.active) swallowReleaseClick();

		const row = chainDropRowIndex !== undefined ? list?.div()?.querySelector(`[data-layer][data-index="${chainDropRowIndex}"]`) : undefined;
		const targetLayerId = chainDropRowIndex !== undefined ? layers[chainDropRowIndex]?.entry.id : undefined;
		if (chainDragState?.active && chainDragDropTarget) {
			// The button's action applies to the selection, so it becomes exactly the dragged nodes
			editor.selectNodes(BigUint64Array.from(chainDragState.nodeIds));
			chainDragDropTarget.dispatchEvent(new CustomEvent("dragdrop"));
		} else if (chainDragState?.active && row && targetLayerId !== undefined && chainInsertIndex !== undefined) {
			const reorderableIds = reorderableChainNodeButtons(row).map((button) => button.getAttribute("data-chain-node-reorderable"));

			// Within the same layer, nodes already together beside the gap would stay put
			const positions = chainDragState.nodeIds.map((id) => reorderableIds.indexOf(String(id))).sort((a, b) => a - b);
			const [first, last] = [positions[0], positions[positions.length - 1]];
			const sameLayer = targetLayerId === chainDragState.layerId;
			const alreadyInPlace = sameLayer && first !== -1 && last - first + 1 === positions.length && chainInsertIndex >= first && chainInsertIndex <= last + 1;

			// The backend counts gaps outward from the layer, the reverse of the left-to-right order shown here
			const nodeIds = BigUint64Array.from(chainDragState.nodeIds);
			const insertIndex = reorderableIds.length - chainInsertIndex;

			// Holding Alt drops copies at the gap instead of moving the originals
			if (e.altKey) editor.duplicateChainNodes(nodeIds, targetLayerId, insertIndex);
			else if (!alreadyInPlace) editor.moveChainNodes(nodeIds, targetLayerId, insertIndex);
		}

		abortChainDrag();
	}

	function abortChainDrag() {
		chainDragState = undefined;
		chainDragActive = false;
		chainDropRowIndex = undefined;
		chainInsertIndex = undefined;
		chainInsertMarkerLeft = undefined;
		chainDragDropTarget = undefined;
		dragInPanel = false;
	}

	function draggingPointerMove(e: PointerEvent) {
		if (chainDragState) {
			chainDraggingPointerMove(e);
			return;
		}

		if (!internalDragState || !list) return;

		// Calculate distance moved
		if (!internalDragState.active) {
			const distance = Math.hypot(e.clientX - internalDragState.startX, e.clientY - internalDragState.startY);
			const DRAG_THRESHOLD = 5;

			if (distance > DRAG_THRESHOLD) {
				internalDragState.active = true;
				dragInPanel = true;
				layerToClipUponClick = undefined;

				const layer = internalDragState.listing.entry;
				if (!$nodeGraph.selected.includes(layer.id)) {
					fakeHighlightOfNotYetSelectedLayerBeingDragged = layer.id;
				}
			}
		}

		// Perform drag calculations if a drag is occurring
		if (internalDragState.active) {
			// Check if the cursor is over any element flagged as a drag drop target
			// (e.g. a bottom-bar action button whose backend widget has an `on_drag_drop` callback set)
			const droppable = (e.target instanceof Element && e.target.closest("[data-drag-droppable~=layers]")) || undefined;
			dragDropTarget = droppable instanceof HTMLElement ? droppable : undefined;

			// Hide the move-in-tree insert indicator whenever the cursor enters the bottom bar
			const overBottomBar = ((e.target instanceof Element && e.target.closest("[data-layer-bottom-bar]")) || undefined) !== undefined;
			if (dragDropTarget || overBottomBar) {
				draggingData = undefined;
				return;
			}

			const select = () => {
				if (internalDragState && !$nodeGraph.selected.includes(internalDragState.layerId)) {
					selectLayer(internalDragState.listing, false, false);
				}
			};

			draggingData = calculateDragIndex(list, e.clientY, select);
		}
	}

	function draggingPointerUp(e: PointerEvent) {
		if (chainDragState) {
			chainDraggingPointerUp(e);
			return;
		}

		if (internalDragState?.active && dragDropTarget) {
			// Ensure the dragged layer is part of the selection, matching the move-in-tree behavior
			if (!$nodeGraph.selected.includes(internalDragState.layerId)) selectLayer(internalDragState.listing, false, false);

			// Hand off to the button's backend `on_drag_drop` callback via the custom event
			dragDropTarget.dispatchEvent(new CustomEvent("dragdrop"));

			swallowReleaseClick();
		} else if (internalDragState?.active && draggingData) {
			const { select, insertParentId, insertIndex } = draggingData;

			// Ensure the dragged layer is part of the selection before committing
			select?.();

			// Holding Alt drops a duplicate of the selection at the target instead of moving the originals
			if (e.altKey) editor.duplicateLayerInTree(insertParentId, insertIndex);
			else editor.moveLayerInTree(insertParentId, insertIndex);

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

	function abortDrag() {
		internalDragState = undefined;
		draggingData = undefined;
		fakeHighlightOfNotYetSelectedLayerBeingDragged = undefined;
		dragInPanel = false;
		dragDropTarget = undefined;
	}

	function draggingMouseDown(e: MouseEvent) {
		// Abort if a drag is active and the user presses the right mouse button (button 2)
		if (e.button === 2 && internalDragState?.active) {
			justFinishedDrag = true;
			abortDrag();
		}
		if (e.button === 2 && chainDragState?.active) {
			justFinishedDrag = true;
			abortChainDrag();
		}
	}

	function draggingKeyDown(e: KeyboardEvent) {
		if (e.key === "Escape" && internalDragState?.active) {
			justFinishedDrag = true;
			abortDrag();
		}
		if (e.key === "Escape" && chainDragState?.active) {
			justFinishedDrag = true;
			abortChainDrag();
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
		<WidgetLayout layout={$layersPanelControlBarLeftLayout} layoutTarget="LayersPanelControlLeftBar" />
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
					<IconButton
						class="status-toggle"
						classes={{ inherited: !listing.parentsVisible }}
						action={(e) => (toggleNodeVisibilityLayerPanel(listing.entry.id), e?.stopPropagation())}
						size={24}
						icon={listing.entry.visible ? "EyeVisible" : "EyeHidden"}
						hoverIcon={listing.entry.visible ? "EyeHide" : "EyeShow"}
						tooltipLabel={listing.entry.visible ? "Hide" : "Show"}
						tooltipDescription={!listing.parentsVisible ? "A parent of this layer is hidden and that status is being inherited." : ""}
						data-drag-toggle-group="layer-visibility"
						data-drag-toggle-state={listing.entry.visible ? "visible" : "hidden"}
					/>
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
					<LayoutRow class="layer-name" classes={{ editing: listing.editingName }} on:dblclick={() => onEditLayerName(listing)}>
						<input
							data-layer-name-input
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
							data-drag-toggle-group="layer-lock"
							data-drag-toggle-state={listing.entry.unlocked ? "unlocked" : "locked"}
						/>
					{:else if activeDragToggleGroup === "layer-lock"}
						<IconButton
							class="status-toggle drag-toggle-placeholder"
							action={(e) => (toggleLayerLock(listing.entry.id), e?.stopPropagation())}
							size={24}
							icon="PadlockUnlocked"
							data-drag-toggle-group="layer-lock"
							data-drag-toggle-state="unlocked"
						/>
					{/if}
					<LayoutRow class="layer-name-spacer" on:dblclick={() => onEditLayerName(listing)} />
					<LayoutRow class="layer-chain-icons" data-layer-chain>
						{#each listing.entry.chainNodes as chainNode, chainNodeIndex}
							<IconButton
								icon={chainNode.icon}
								size={24}
								classes={{ selected: chainNode.selected, hidden: !chainNode.visible }}
								action={(e) => chainNodeClick(e, listing, chainNode.id)}
								tooltipLabel={chainNode.name}
								data-chain-node={chainNodeIndex}
								data-chain-node-reorderable={chainNode.reorderable ? String(chainNode.id) : undefined}
							/>
						{/each}
						{#if listing.entry.chainNodes.length > 0}
							<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8" class="chain-connector" style:fill={`var(--color-data-${listing.entry.chainDataType.toLowerCase()})`}>
								<path d="M0,6.306L0,1.694C0,0.228 1.06,-0.41 2.356,0.276L7.028,2.752C8.324,3.438 8.324,4.562 7.028,5.248L2.356,7.723C1.06,8.41 0,7.771 0,6.306z" />
							</svg>
						{/if}
						<IconLabel icon={listing.entry.iconName} class="layer-type-icon" classes={{ hidden: !listing.entry.visible }} tooltipLabel={listing.entry.implementationName} />
					</LayoutRow>
					{#if chainDropRowIndex === index && chainInsertMarkerLeft !== undefined}
						<div class="chain-insert-mark" style:left={`${chainInsertMarkerLeft}px`}></div>
					{/if}
				</LayoutRow>
			{/each}
		</LayoutCol>
		{#if draggingData && !draggingData.highlightFolder && dragInPanel}
			<div class="insert-mark" style:left={`${4 + 24 + draggingData.insertDepth * 16}px`} style:top={`${draggingData.markerHeight}px`}></div>
		{/if}
	</LayoutRow>
	<LayoutRow class="bottom-bar" classes={{ "layer-drag-active": Boolean(internalDragState?.active), "chain-node-drag-active": chainDragActive }} scrollableX={true} data-layer-bottom-bar>
		<WidgetLayout layout={$layersPanelBottomBarLeftLayout} layoutTarget="LayersPanelBottomLeftBar" />
		<WidgetLayout layout={$layersPanelBottomBarRightLayout} layoutTarget="LayersPanelBottomRightBar" />
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
			justify-content: space-between;
			border-top: 1px solid var(--color-2-mildblack);

			.widget-span > * {
				margin: 0;
			}

			&:not(:has(*)) {
				display: none;
			}

			// While dragging, buttons are grayed out unless they accept the kind of item being dragged
			&.layer-drag-active,
			&.chain-node-drag-active {
				.icon-button,
				.popover-button {
					pointer-events: none;
					background: none;

					svg {
						fill: var(--color-8-uppergray);
					}
				}
			}

			// Dropped items become the selection before the button acts, so a button disabled for the current selection still accepts them
			&.layer-drag-active .icon-button[data-drag-droppable~="layers"],
			&.chain-node-drag-active .icon-button[data-drag-droppable~="chain-nodes"] {
				pointer-events: auto;

				svg {
					fill: var(--color-e-nearwhite);
				}

				&:hover {
					background: var(--color-e-nearwhite);

					svg {
						fill: var(--color-2-mildblack);
					}
				}
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
					background: rgb(from var(--color-4-dimgray) r g b / 0.5);
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

				// Indents the rest of the row by the layer's depth in the tree, after the visibility column
				.expand-arrow,
				.expand-arrow-none {
					margin-left: calc(var(--layer-indent-levels) * 16px);
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

				// Unrelated-width gap from the name (with its right margin), and the layer icon kept 4px from the row's end like the visibility icon at its start
				.layer-chain-icons {
					flex: 0 0 auto;
					align-items: center;
					margin-left: 4px;
					margin-right: 2px;

					.icon-button {
						width: 20px;
					}

					.icon-button.selected {
						background: var(--color-6-lowergray);
						z-index: 1;
					}

					// A hidden node's icon is faded and struck through by a slash across its 16px icon's diagonal
					.icon-button.hidden,
					.layer-type-icon.hidden {
						position: relative;

						svg {
							opacity: 0.5;
						}

						&::after {
							content: "";
							position: absolute;
							left: 50%;
							top: 50%;
							width: calc(16px * sqrt(2) - 2px);
							height: 1px;
							background: var(--color-e-nearwhite);
							transform: translate(-50%, -50%) rotate(-45deg);
							pointer-events: none;
						}
					}

					// Related-width gaps to the icons beside it
					.chain-connector {
						flex: 0 0 auto;
						align-self: center;
						width: 4px;
						height: 4px;
						margin: 0 2px;
					}

					// Takes the same 20px width as the chain buttons
					.layer-type-icon {
						margin: 0 2px;
						align-self: center;
					}
				}

				// The vertical counterpart of the insertion marker between layers, centered on the gap between chain nodes
				.chain-insert-mark {
					position: absolute;
					top: 4px;
					bottom: 4px;
					width: 5px;
					transform: translateX(-50%);
					background: var(--color-e-nearwhite);
					// Above the selected chain node's raised highlight
					z-index: 2;
					pointer-events: none;
				}

				// Pushes the chain icons to the row's end, and can also be double-clicked to rename the layer like the name itself
				.layer-name-spacer {
					flex: 1 1 0;
					align-self: stretch;
				}

				// Only as wide as its text, so the lock icon follows right after it, but shrinks to an ellipsis when space runs out
				.layer-name {
					flex: 0 1 auto;
					margin: 0 8px;

					// While renaming, the input stretches to fill the row
					&.editing {
						flex-grow: 1;

						input {
							width: 100%;
						}

						& ~ .layer-name-spacer {
							flex-grow: 0;
						}
					}

					input {
						field-sizing: content;
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

						&:disabled {
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

					// Invisible placeholder rendered only during a lock drag-toggle gesture, so the drag can still land on rows whose lock icon is normally omitted
					&.drag-toggle-placeholder {
						opacity: 0; // Not `visibility: hidden`, which would exclude it from hit-testing
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

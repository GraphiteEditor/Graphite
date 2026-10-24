<script lang="ts">
	import { getContext, onMount, onDestroy, tick } from "svelte";
	import LayoutCol from "/src/components/layout/LayoutCol.svelte";
	import LayoutRow from "/src/components/layout/LayoutRow.svelte";
	import IconButton from "/src/components/widgets/buttons/IconButton.svelte";
	import IconLabel from "/src/components/widgets/labels/IconLabel.svelte";
	import WidgetLayout from "/src/components/widgets/WidgetLayout.svelte";
	import { createDragToggleManager, destroyDragToggleManager } from "/src/managers/drag-toggle";
	import type { NodeGraphStore } from "/src/stores/node-graph";
	import { layersPanelControlBarLeftLayout, layersPanelBottomBarLeftLayout, layersPanelBottomBarRightLayout, retainLayerDetails } from "/src/stores/portfolio";
	import type { PortfolioStore } from "/src/stores/portfolio";
	import type { TooltipStore } from "/src/stores/tooltip";
	import { createAutoScroller, speedWhileAimingAt } from "/src/utility-functions/auto-scroll";
	import { pasteFile } from "/src/utility-functions/files";
	import { targetIsTextField } from "/src/utility-functions/input";
	import { operatingSystem } from "/src/utility-functions/platform";
	import type { EditorWrapper, LayerPanelChainInsertion, LayerPanelChainNode, LayerPanelEntry, LayerStructureEntry } from "/wrapper/pkg/graphite_wasm_wrapper";

	type LayerListingInfo = {
		id: bigint;
		folderIndex: number;
		bottomLayer: boolean;
		editingName: boolean;
		// Known only while the listing is in view, since the backend sends the details of just those layers
		entry: LayerPanelEntry | undefined;
		depth: number;
		parentId: bigint | undefined;
		// Along this listing's own path, since a layer listed under several parents can inherit a hide or lock from just one
		parentsVisible: boolean;
		parentsUnlocked: boolean;
		childrenPresent: boolean;
		expanded: boolean;
		selected: boolean;
		ancestorOfSelected: boolean;
		treePath: bigint[];
		// Where the listing's rows start in the list, and how tall they are together with those of its expanded chain
		top: number;
		height: number;
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

	// What a drag reads from a pointer move, which auto-scrolling also gives when it redoes the drag where the pointer last was
	type DragPointer = Pick<PointerEvent, "clientX" | "clientY" | "target">;

	// One place along a layer's chain: one of its nodes (with its place among the reorderable ones, if it is one), or the ghost of a node the active tool would add there
	type ChainSlot = { kind: "node"; node: LayerPanelChainNode; nodeIndex: number; reorderablePosition: number | undefined } | { kind: "insertion"; insertion: LayerPanelChainInsertion };

	// The run of a chain's slots, from `start` up to but not including `end`, that an ellipsis stands in for, and the gaps among the reorderable nodes at its two edges
	type ChainEllipsis = { start: number; end: number; nodes: LayerPanelChainNode[]; gapBefore: number | undefined; gapAfter: number | undefined };

	// The width of each chain slot (a node button, a ghost node, or the ellipsis), and of the expand arrow and layer icon that end the chain, as styled below
	const CHAIN_SLOT_WIDTH = 20;
	const CHAIN_EXPAND_ARROW_WIDTH = 12;
	const LAYER_ICON_WIDTH = 20;

	// The height of a layer's row and of a row listing a node of its expanded chain, each with its bottom border, as styled below
	const LAYER_ROW_HEIGHT = 33;
	const CHAIN_NODE_ROW_HEIGHT = 25;
	// Rows this far past either end of the list's visible part are rendered too, so scrolling doesn't uncover them before they appear
	const RENDERED_BEYOND_VIEW_PIXELS = 240;

	const editor = getContext<EditorWrapper>("editor");
	const nodeGraph = getContext<NodeGraphStore>("nodeGraph");
	const tooltip = getContext<TooltipStore>("tooltip");
	const portfolio = getContext<PortfolioStore>("portfolio");

	let listArea: LayoutRow | undefined;
	let bottomBar: LayoutRow | undefined;
	let list: LayoutCol | undefined;

	let layers: LayerListingInfo[] = [];
	let layersBuiltFrom: LayerStructureEntry[] | undefined = undefined;

	// Rows are rendered only for the listings from `firstRendered` up to but not including `endRendered`, with the rest of the list's height left empty around them
	let firstRendered = 0;
	let endRendered = 0;
	let listHeight = 0;
	// The document and layers the backend was last told have rows rendered, which it sends the details and thumbnails of
	let renderedDocumentSent: bigint | undefined = undefined;
	let renderedLayersSent = "";
	const listAreaResizeObserver = new ResizeObserver(() => updateRenderedRows());

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
	// The layer (by row index) whose chain is under the cursor, the gap among its reorderable nodes counted from the left, and the marker's spot in its row or node rows
	let chainDropRowIndex: number | undefined = undefined;
	let chainInsertIndex: number | undefined = undefined;
	let chainInsertMarkerLeft: number | undefined = undefined;
	let chainRowInsertMarker: { nodeIndex: number; edge: "top" | "bottom" } | undefined = undefined;
	let chainDragDropTarget: HTMLElement | undefined = undefined;

	// Scrolling the list near its edges during a drag shifts the content under the pointer, so the drag is redone where the pointer last was
	let lastDragPointer: { clientX: number; clientY: number } | undefined = undefined;
	const autoScroller = createAutoScroller(() => {
		if (!lastDragPointer) return;
		const { clientX, clientY } = lastDragPointer;

		if (internalDragState || chainDragState) draggingPointerMove({ clientX, clientY, target: document.elementFromPoint(clientX, clientY) });
		else if (list) draggingData = calculateDragIndex(list, clientY);
	});

	// Viewport hover outline, tracked by row index since rows get reused when the layers change beneath a still pointer
	let hoveredRowIndex: number | undefined = undefined;
	let hoveredLayerSent: bigint | undefined = undefined;
	// Scrolling carries rows beneath a pointer that hasn't moved, which doesn't count as hovering them until it moves again
	let hoverPausedByScroll = false;

	// Interactive clipping
	let layerToClipUponClick: LayerListingInfo | undefined = undefined;
	let layerToClipAltKeyPressed = false;

	// Drag-toggle: tracked here so the template can render the invisible lock placeholder during a `layer-lock` gesture
	let activeDragToggleGroup: string | undefined = undefined;

	// The node being renamed in an expanded chain, by the row of the layer listing it, since a node shared by several chains is listed in each
	let editingChainNodeName: { rowIndex: number; nodeId: bigint } | undefined = undefined;

	// Each row's chain width, which the grid sets from the room the layer name leaves it, deciding which nodes the ellipsis stands in for
	let chainWidths: number[] = [];
	const chainResizeObserver = new ResizeObserver((entries) => {
		entries.forEach((entry) => {
			const rowIndex = Number(entry.target.closest("[data-layer]")?.getAttribute("data-index"));
			if (!Number.isNaN(rowIndex)) chainWidths[rowIndex] = entry.contentRect.width;
		});
		chainWidths = chainWidths;
	});

	$: rebuildLayerHierarchy($portfolio.layerStructure);
	$: attachLayerDetails($portfolio.layerCache);
	$: sendHoveredLayer(layers, hoveredRowIndex);

	onMount(() => {
		createDragToggleManager(dragToggleListener);

		const listAreaDiv = listArea?.div();
		if (listAreaDiv) listAreaResizeObserver.observe(listAreaDiv);

		addEventListener("pointerup", draggingPointerUp);
		addEventListener("pointermove", draggingPointerMove);
		addEventListener("pointercancel", abortDrags);
		addEventListener("blur", abortDrags);
		addEventListener("mousedown", draggingMouseDown);
		addEventListener("keydown", draggingKeyDown);
		addEventListener("keydown", handleLayerPanelKeyDown);

		addEventListener("pointermove", clippingHover);
		addEventListener("keydown", clippingKeyPress);
		addEventListener("keyup", clippingKeyPress);

		addEventListener("pointermove", resumeHoverAfterScroll);
	});

	onDestroy(() => {
		destroyDragToggleManager(dragToggleListener);

		removeEventListener("pointerup", draggingPointerUp);
		removeEventListener("pointermove", draggingPointerMove);
		removeEventListener("pointercancel", abortDrags);
		removeEventListener("blur", abortDrags);
		removeEventListener("mousedown", draggingMouseDown);
		removeEventListener("keydown", draggingKeyDown);
		removeEventListener("keydown", handleLayerPanelKeyDown);

		removeEventListener("pointermove", clippingHover);
		removeEventListener("keydown", clippingKeyPress);
		removeEventListener("keyup", clippingKeyPress);

		removeEventListener("pointermove", resumeHoverAfterScroll);

		chainResizeObserver.disconnect();
		listAreaResizeObserver.disconnect();
		stopAutoScroll();

		// With no rows left, the backend stops sending details, so the ones kept here would go stale
		editor.setLayersPanelView(new BigUint64Array(), new BigUint64Array());
		retainLayerDetails(new Set(), false);

		// Closing the panel removes the hovered row without a `pointerleave`
		if (hoveredLayerSent !== undefined) editor.setLayersPanelHoveredLayer(undefined);
	});

	// Renders the rows of just the listings in or near view, and tells the backend which layers those are so it sends only their details and thumbnails
	function updateRenderedRows() {
		const container = listArea?.div();
		const listElement = list?.div();
		if (!container || !listElement) return;

		// Measured from the top of the list's first row
		const renderedTop = container.scrollTop - listElement.offsetTop - RENDERED_BEYOND_VIEW_PIXELS;
		const renderedBottom = container.scrollTop - listElement.offsetTop + container.clientHeight + RENDERED_BEYOND_VIEW_PIXELS;

		const [previousFirstRendered, previousEndRendered] = [firstRendered, endRendered];
		firstRendered = firstListingWhere((listing) => listing.top + listing.height > renderedTop);
		endRendered = firstListingWhere((listing) => listing.top >= renderedBottom);

		// Listings no longer rendered let go of their details, which the backend stops keeping current, and of renaming, since their removed input can't lose focus to end it
		layers.slice(previousFirstRendered, previousEndRendered).forEach((listing, offset) => {
			const index = previousFirstRendered + offset;
			if (index >= firstRendered && index < endRendered) return;

			listing.entry = undefined;
			if (listing.editingName) {
				listing.editingName = false;
				draggable = true;
			}
		});

		attachLayerDetails($portfolio.layerCache);
		sendRenderedLayers();
	}

	// The index of the first listing meeting a condition that every later listing meets too, found in O(log n)
	function firstListingWhere(condition: (listing: LayerListingInfo) => boolean): number {
		let low = 0;
		let high = layers.length;
		while (low < high) {
			const middle = Math.floor((low + high) / 2);
			if (condition(layers[middle])) high = middle;
			else low = middle + 1;
		}
		return low;
	}

	function sendRenderedLayers() {
		const documentId = $portfolio.documents[$portfolio.activeDocumentIndex]?.id;
		const renderedLayers = layers.slice(firstRendered, endRendered).map((listing) => listing.id);
		const key = renderedLayers.join(",");
		if (documentId === renderedDocumentSent && key === renderedLayersSent) return;
		const otherDocument = documentId !== renderedDocumentSent;
		renderedDocumentSent = documentId;
		renderedLayersSent = key;

		// The backend keeps only these layers' details current, so the others kept here would go stale
		retainLayerDetails(new Set(renderedLayers.map(String)), otherDocument);
		const missingDetails = renderedLayers.filter((id) => !$portfolio.layerCache.has(String(id)));
		editor.setLayersPanelView(BigUint64Array.from(renderedLayers), BigUint64Array.from(missingDetails));
	}

	// O(rendered rows)
	function attachLayerDetails(cache: Map<string, LayerPanelEntry>) {
		layers.slice(firstRendered, endRendered).forEach((listing) => {
			listing.entry = cache.get(String(listing.id));
		});
		layers = layers;
	}

	// Brings a listing's rows into view, rendering them before it resolves
	async function scrollListingIntoView(listing: LayerListingInfo) {
		const container = listArea?.div();
		const listElement = list?.div();
		if (!container || !listElement) return;

		const top = listElement.offsetTop + listing.top;
		if (top < container.scrollTop) container.scrollTop = top;
		else if (top + LAYER_ROW_HEIGHT > container.scrollTop + container.clientHeight) container.scrollTop = top + LAYER_ROW_HEIGHT - container.clientHeight;

		updateRenderedRows();
		await tick();
	}

	function listAreaScroll() {
		pauseHoverForScroll();
		updateRenderedRows();
	}

	function hoverRow(rowIndex: number) {
		if (!hoverPausedByScroll) hoveredRowIndex = rowIndex;
	}

	function pauseHoverForScroll() {
		hoverPausedByScroll = true;
		hoveredRowIndex = undefined;
	}

	// The pointer's first actual move after a scroll hovers whichever row it's over, which it may have entered while hovering was paused
	function resumeHoverAfterScroll(e: PointerEvent) {
		if (!hoverPausedByScroll || (e.movementX === 0 && e.movementY === 0)) return;
		hoverPausedByScroll = false;

		const row = e.target instanceof Element ? e.target.closest("[data-layer], [data-chain-row-of]") : undefined;
		if (!row || !list?.div()?.contains(row)) return;
		hoveredRowIndex = Number(row.getAttribute("data-chain-row-of") || row.getAttribute("data-index"));
	}

	function sendHoveredLayer(listings: LayerListingInfo[], rowIndex: number | undefined) {
		const id = rowIndex === undefined ? undefined : listings.at(rowIndex)?.id;
		if (id === hoveredLayerSent) return;

		hoveredLayerSent = id;
		editor.setLayersPanelHoveredLayer(id);
	}

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

		// A layer chosen from beyond the rendered rows, like the next one by keyboard, needs its row before its name can be edited
		await scrollListingIntoView(listing);

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
		editor.setLayerName(listing.id, name);
		if (listing.entry) listing.entry.alias = name;
	}

	async function onEditLayerNameDeselect(listing: LayerListingInfo) {
		draggable = true;
		listing.editingName = false;
		layers = layers;

		// Set it back to the original name if the user didn't enter a new name
		if (document.activeElement instanceof HTMLInputElement) document.activeElement.value = listing.entry?.alias || "";

		// Deselect the text so it doesn't appear selected while the input field becomes disabled and styled to look like regular text
		window.getSelection()?.removeAllRanges();
	}

	async function onEditChainNodeName(rowIndex: number, nodeId: bigint) {
		draggable = false;
		editingChainNodeName = { rowIndex, nodeId };

		await tick();

		const query = list?.div?.()?.querySelector("[data-chain-node-name-input]:enabled");
		const textInput = (query instanceof HTMLInputElement && query) || undefined;
		textInput?.select();
	}

	function onEditChainNodeNameChange(nodeId: bigint, e: Event) {
		// Eliminate duplicate events
		if (!editingChainNodeName) return;

		draggable = true;
		editingChainNodeName = undefined;

		const name = (e.target instanceof HTMLInputElement && e.target.value) || "";
		editor.setLayerName(nodeId, name);
	}

	function onEditChainNodeNameDeselect(chainNode: LayerPanelChainNode) {
		draggable = true;
		editingChainNodeName = undefined;

		// Set it back to the original name if the user didn't enter a new name
		if (document.activeElement instanceof HTMLInputElement) document.activeElement.value = chainNode.name;

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
		if (layerToClipAltKeyPressed && layerToClipUponClick?.entry?.clippable) clipLayer(layerToClipUponClick);
		// Select the layer only if the accel and/or shift keys are pressed
		else if (!oppositeAccel && !alt) selectLayer(listing, accel, shift);

		e.stopPropagation();
	}

	function clipLayer(listing: LayerListingInfo) {
		editor.clipLayer(listing.id);
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

		editor.selectLayer(listing.id, accel, shift);
	}

	function chainNodeClick(e: MouseEvent | undefined, listing: LayerListingInfo, nodeId: bigint) {
		e?.stopPropagation();

		// The click that follows releasing a drag shouldn't select anything
		if (justFinishedDrag) return;

		// Same modifier key handling as selecting layers
		const [accel, oppositeAccel] = operatingSystem() === "Mac" ? [e?.metaKey, e?.ctrlKey] : [e?.ctrlKey, e?.metaKey];
		if (oppositeAccel || e?.altKey) return;

		editor.selectChainNode(listing.id, nodeId, Boolean(accel), Boolean(e?.shiftKey));
	}

	// Double-clicking a chain node's icon, or the layer's own icon at the row's end (unless it expands the chain), opens the graph centered on that node
	function chainIconsDoubleClick(e: MouseEvent, listing: LayerListingInfo) {
		const target = e.target instanceof Element ? e.target : undefined;
		const chainNodeButton = target?.closest("[data-chain-node]");
		const chainNode = chainNodeButton ? listing.entry?.chainNodes[Number(chainNodeButton.getAttribute("data-chain-node"))] : undefined;

		if (chainNode) editor.showNodeInGraph(chainNode.id);
		else if (target?.closest("[data-layer-icon]") && !target.closest("[data-chain-expandable]")) editor.showNodeInGraph(listing.id);
	}

	// The kind of node it is, followed by its name if it's been renamed
	function chainNodeTooltipLabel(node: LayerPanelChainNode): string {
		return node.name === node.implementationName ? node.name : `${node.implementationName} (${node.name})`;
	}

	function observeChainWidth(element: HTMLElement) {
		chainResizeObserver.observe(element);
		return { destroy: () => chainResizeObserver.unobserve(element) };
	}

	function chainSlots(entry: LayerPanelEntry): ChainSlot[] {
		const insertion = entry.toolTargetInsertion;
		const slots: ChainSlot[] = [];

		let reorderableCount = 0;
		entry.chainNodes.forEach((node, nodeIndex) => {
			if (insertion?.index === nodeIndex) slots.push({ kind: "insertion", insertion });
			slots.push({ kind: "node", node, nodeIndex, reorderablePosition: node.reorderable ? reorderableCount++ : undefined });
		});
		if (insertion?.index === entry.chainNodes.length) slots.push({ kind: "insertion", insertion });

		return slots;
	}

	// The chain's width with every inline slot shown, which it gets whenever the layer name leaves enough room
	function chainFullWidth(slotCount: number, withArrow: boolean): number {
		return slotCount * CHAIN_SLOT_WIDTH + (withArrow ? CHAIN_EXPAND_ARROW_WIDTH : 0) + LAYER_ICON_WIDTH;
	}

	// Narrower than the ellipsis alone (with the expand arrow and layer icon after it), the chain shows nothing rather than a clipped part
	function chainFits(slotCount: number, withArrow: boolean, width: number | undefined): boolean {
		return width === undefined || width >= chainFullWidth(Math.min(slotCount, 1), withArrow);
	}

	function chainEllipsis(slots: ChainSlot[], width: number | undefined): ChainEllipsis | undefined {
		if (width === undefined) return undefined;

		// Of the slots that fit, one goes to the ellipsis itself
		const fittingCount = Math.floor((width - CHAIN_EXPAND_ARROW_WIDTH - LAYER_ICON_WIDTH) / CHAIN_SLOT_WIDTH);
		if (fittingCount >= slots.length) return undefined;
		const shownCount = Math.max(fittingCount - 1, 0);
		const hiddenCount = slots.length - shownCount;

		// The hidden run starts at the middle, sparing both ends once two or more slots are shown
		const middle = Math.ceil(shownCount / 2);
		let start = middle;

		// It slides off the node the active tool edits (or the ghost of one it adds) to whichever side keeps it nearer the middle, if either still spares both ends
		const pinned = slots.findIndex((slot) => slot.kind === "insertion" || Boolean(slot.node.toolTargetDescription) || slot.node.editInProgress);
		if (pinned >= start && pinned < start + hiddenCount) {
			const candidates = [pinned + 1, pinned - hiddenCount].filter((candidate) => candidate >= 1 && candidate + hiddenCount <= slots.length - 1);
			if (candidates.length > 0) start = candidates.reduce((best, candidate) => (Math.abs(candidate - middle) < Math.abs(best - middle) ? candidate : best));
		}
		const end = start + hiddenCount;

		const hiddenNodes = slots.slice(start, end).flatMap((slot) => (slot.kind === "node" ? [slot] : []));
		const reorderablePositions = hiddenNodes.flatMap((slot) => (slot.reorderablePosition === undefined ? [] : [slot.reorderablePosition]));
		const lastReorderablePosition = reorderablePositions.at(-1);

		return {
			start,
			end,
			nodes: hiddenNodes.map((slot) => slot.node),
			gapBefore: reorderablePositions.at(0),
			gapAfter: lastReorderablePosition === undefined ? undefined : lastReorderablePosition + 1,
		};
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
				// A row of an expanded chain's nodes counts as the bottom of its layer's row
				const chainRowOf = treeChild.getAttribute("data-chain-row-of");
				const indexAttribute = treeChild.getAttribute("data-index") || chainRowOf;
				if (!indexAttribute) return;
				const listing = layers[parseInt(indexAttribute, 10)];

				const rect = treeChild.getBoundingClientRect();
				if (rect.top > clientY || rect.bottom < clientY) {
					return;
				}
				const pointerPercentage = chainRowOf ? 1 : (clientY - rect.top) / rect.height;

				// Dropping after the layer lands below its chain's node rows, if they're shown
				const lastChainRow = Array.from(treeChildren).findLast((child) => child.getAttribute("data-chain-row-of") === indexAttribute);
				const bottom = (lastChainRow || treeChild).getBoundingClientRect().bottom;
				if (listing.entry?.childrenAllowed || listing.childrenPresent) {
					if (pointerPercentage < 0.25) {
						insertParentId = listing.parentId;
						insertDepth = listing.depth - 1;
						insertIndex = listing.folderIndex;
						markerHeight = rect.top - layerPanelTop;
					} else if (pointerPercentage < 0.75 || (listing.childrenPresent && listing.expanded)) {
						insertParentId = listing.id;
						insertDepth = listing.depth;
						insertIndex = 0;
						highlightFolder = true;
						highlightFolderIndex = parseInt(indexAttribute, 10);
					} else {
						insertParentId = listing.parentId;
						insertDepth = listing.depth - 1;
						insertIndex = listing.folderIndex + 1;
						markerHeight = bottom - layerPanelTop;
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
						markerHeight = bottom - layerPanelTop;
					}
				}
			});
			// Dragging to the empty space below all layers, which only follows the last row once that's rendered
			const lastLayer = treeChildren[treeChildren.length - 1];
			if (endRendered === layers.length && lastLayer && lastLayer.getBoundingClientRect().bottom < clientY) {
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
			const chainNode = listing.entry?.chainNodes[Number(chainNodeButton.getAttribute("data-chain-node"))];
			if (chainNode) chainNodePointerDown(e, listing, chainNode);
			return;
		}

		internalDragState = {
			active: false,
			layerId: listing.id,
			listing: listing,
			startX: e.clientX,
			startY: e.clientY,
		};
	}

	function chainNodePointerDown(e: PointerEvent, listing: LayerListingInfo, chainNode: LayerPanelChainNode) {
		// A source node stays fixed at the chain's upstream end
		if (!chainNode.reorderable) return;

		// Dragging a selected node brings along the chain's other selected nodes, which all end up together at the drop
		const chainNodes = listing.entry?.chainNodes || [];
		const nodeIds = chainNode.selected ? chainNodes.filter((node) => node.reorderable && node.selected).map((node) => node.id) : [chainNode.id];

		chainDragState = { active: false, layerId: listing.id, nodeIds, startX: e.clientX, startY: e.clientY };
	}

	// Scrolls the list while a drag's pointer is near or past its top or bottom edge, easing off while it heads from where it began toward a bottom bar button it can be dropped on
	function autoScrollDrag(clientX: number, clientY: number, aim?: { start: [number, number]; droppable: string }) {
		lastDragPointer = { clientX, clientY };
		const container = listArea?.div();
		if (!container) return;

		const dropTargets = aim && dropTargetBounds(aim.droppable);
		const speed = aim && dropTargets ? speedWhileAimingAt(aim.start, clientX, clientY, dropTargets) : 1;
		autoScroller.update(container, clientX, clientY, speed);
	}

	// The area spanned by the bottom bar buttons that accept the kind of item being dragged
	function dropTargetBounds(droppable: string): DOMRect | undefined {
		const buttons = Array.from(bottomBar?.div()?.querySelectorAll(`[data-drag-droppable~="${droppable}"]`) || []);
		if (buttons.length === 0) return undefined;

		const rects = buttons.map((button) => button.getBoundingClientRect());
		const left = Math.min(...rects.map((rect) => rect.left));
		const top = Math.min(...rects.map((rect) => rect.top));
		const right = Math.max(...rects.map((rect) => rect.right));
		const bottom = Math.max(...rects.map((rect) => rect.bottom));
		return new DOMRect(left, top, right - left, bottom - top);
	}

	function stopAutoScroll() {
		lastDragPointer = undefined;
		autoScroller.stop();
	}

	function chainDraggingPointerMove(e: DragPointer) {
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
		chainRowInsertMarker = undefined;

		// Over a bottom bar button accepting chain nodes, the drop goes to it instead, and elsewhere in the bottom bar it goes nowhere
		const target = e.target instanceof Element ? e.target : undefined;
		const droppable = target?.closest("[data-drag-droppable~=chain-nodes]");
		chainDragDropTarget = droppable instanceof HTMLElement ? droppable : undefined;
		autoScrollDrag(e.clientX, e.clientY, { start: [chainDragState.startX, chainDragState.startY], droppable: "chain-nodes" });
		if (chainDragDropTarget || target?.closest("[data-layer-bottom-bar]")) return;

		// Otherwise the nodes drop into the chain of whichever layer is under the cursor, or whose expanded chain's node row is
		const nodeRow = target?.closest("[data-chain-row-of]");
		const row = target?.closest("[data-layer]");
		if (nodeRow) {
			calculateChainRowInsertIndex(Number(nodeRow.getAttribute("data-chain-row-of")), e.clientY);
		} else if (row) {
			const rowIndex = Number(row.getAttribute("data-index"));
			const entry = layers[rowIndex]?.entry;

			// An expanded chain's nodes are in the rows beneath the layer, so the drop goes atop them, nearest the layer
			if (entry?.chainExpanded && entry.chainNodes.length > 0) calculateChainRowInsertIndex(rowIndex, row.getBoundingClientRect().bottom);
			else calculateChainInsertIndex(row, e.clientX);
		}
	}

	function calculateChainInsertIndex(row: Element, clientX: number) {
		// Each edge of a shown reorderable node, or of the ellipsis standing in for hidden ones, marks a gap among the chain's reorderable nodes
		const edges = Array.from(row.querySelectorAll("[data-chain-gap-before]")).flatMap((element) => {
			const rect = element.getBoundingClientRect();
			return [
				{ x: rect.left, gap: Number(element.getAttribute("data-chain-gap-before")) },
				{ x: rect.right, gap: Number(element.getAttribute("data-chain-gap-after")) },
			];
		});

		// The drop lands at the edge nearest the cursor, where the marker sits.
		// With no reorderable node shown, it goes beside the layer, after a fixed source node or else just before the layer's icon.
		let markerX: number | undefined;
		let index: number;
		if (edges.length === 0) {
			index = layers[Number(row.getAttribute("data-index"))]?.entry?.chainNodes.filter((node) => node.reorderable).length || 0;
			const chainNodeButtons = row.querySelectorAll("[data-chain-node]");
			const fixedSource = chainNodeButtons[chainNodeButtons.length - 1];
			markerX = fixedSource ? fixedSource.getBoundingClientRect().right : row.querySelector("[data-layer-chain]")?.getBoundingClientRect().left;
		} else {
			const nearest = edges.reduce((best, edge) => (Math.abs(edge.x - clientX) < Math.abs(best.x - clientX) ? edge : best));
			markerX = nearest.x;
			index = nearest.gap;
		}
		if (markerX === undefined) return;

		chainDropRowIndex = Number(row.getAttribute("data-index"));
		chainInsertIndex = index;
		chainInsertMarkerLeft = markerX - row.getBoundingClientRect().left;
	}

	function calculateChainRowInsertIndex(rowIndex: number, clientY: number) {
		const nodeRows = Array.from(list?.div()?.querySelectorAll(`[data-chain-row-of="${rowIndex}"]`) || []);
		if (nodeRows.length === 0) return;

		// Listed downstream first, a reorderable node's row has the gap after it on its top edge and the gap before it on its bottom edge
		const edges = nodeRows.flatMap((nodeRow) => {
			if (!nodeRow.hasAttribute("data-chain-gap-before")) return [];
			const gapBefore = Number(nodeRow.getAttribute("data-chain-gap-before"));

			const rect = nodeRow.getBoundingClientRect();
			const nodeIndex = Number(nodeRow.getAttribute("data-chain-row-node"));
			return [
				{ y: rect.top, gap: gapBefore + 1, marker: { nodeIndex, edge: "top" as const } },
				{ y: rect.bottom, gap: gapBefore, marker: { nodeIndex, edge: "bottom" as const } },
			];
		});

		// As in the layer's row, the drop lands at the nearest edge, or with no reorderable node, atop the fixed source node's row
		if (edges.length === 0) {
			chainInsertIndex = 0;
			chainRowInsertMarker = { nodeIndex: Number(nodeRows[0].getAttribute("data-chain-row-node")), edge: "top" };
		} else {
			const nearest = edges.reduce((best, edge) => (Math.abs(edge.y - clientY) < Math.abs(best.y - clientY) ? edge : best));
			chainInsertIndex = nearest.gap;
			chainRowInsertMarker = nearest.marker;
		}
		chainDropRowIndex = rowIndex;
	}

	// Swallow the click fired by releasing a drag, without leaving the flag set to block a later click if none comes
	function swallowReleaseClick() {
		justFinishedDrag = true;
		setTimeout(() => (justFinishedDrag = false), 0);
	}

	function chainDraggingPointerUp(e: PointerEvent) {
		if (chainDragState?.active) swallowReleaseClick();

		const targetEntry = chainDropRowIndex !== undefined ? layers[chainDropRowIndex]?.entry : undefined;
		const targetLayerId = targetEntry?.id;
		if (chainDragState?.active && chainDragDropTarget) {
			// The button's action applies to the selection, so it becomes exactly the dragged nodes
			editor.selectNodes(BigUint64Array.from(chainDragState.nodeIds));
			chainDragDropTarget.dispatchEvent(new CustomEvent("dragdrop"));
		} else if (chainDragState?.active && targetEntry && targetLayerId !== undefined && chainInsertIndex !== undefined) {
			// All the chain's reorderable nodes, including any the ellipsis stands in for
			const reorderableIds = targetEntry.chainNodes.flatMap((node) => (node.reorderable ? [node.id] : []));

			// Within the same layer, nodes already together beside the gap would stay put
			const positions = chainDragState.nodeIds.map((id) => reorderableIds.indexOf(id)).sort((a, b) => a - b);
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
		chainRowInsertMarker = undefined;
		chainDragDropTarget = undefined;
		dragInPanel = false;
		stopAutoScroll();
	}

	function draggingPointerMove(e: DragPointer) {
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

				const layerId = internalDragState.listing.id;
				if (!$nodeGraph.selected.includes(layerId)) {
					fakeHighlightOfNotYetSelectedLayerBeingDragged = layerId;
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
			autoScrollDrag(e.clientX, e.clientY, { start: [internalDragState.startX, internalDragState.startY], droppable: "layers" });
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
		stopAutoScroll();
	}

	// A drag cut short by the pointer being canceled or the window losing focus won't get the release that ends it
	function abortDrags() {
		if (internalDragState) abortDrag();
		if (chainDragState) abortChainDrag();
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

		// Only handle F2 if not currently editing a layer name or typing in another text field
		if (e.key === "F2" && !layers.some((layer) => layer.editingName) && !targetIsTextField(e.target || undefined)) {
			// Find the first selected layer
			const selectedLayer = layers.find((layer) => layer.selected);
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
			editor.setLayerName(currentListing.id, name);
			if (currentListing.entry) currentListing.entry.alias = name;
		}

		// Find current layer index
		const currentIndex = layers.indexOf(currentListing);
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
		autoScrollDrag(e.clientX, e.clientY);
	}

	function fileDragLeave(e: DragEvent) {
		// Crossing between the list's own elements, as auto-scrolling content does beneath the pointer, doesn't leave the list where files are dropped
		const listAreaDiv = listArea?.div();
		if (listAreaDiv && e.relatedTarget instanceof Node && listAreaDiv.contains(e.relatedTarget)) return;

		dragInPanel = false;
		stopAutoScroll();
	}

	function fileDrop(e: DragEvent) {
		if (!draggingData || !e.dataTransfer || !e.dataTransfer.types.includes("Files")) return;

		const { insertParentId, insertIndex } = draggingData;

		e.preventDefault();

		Array.from(e.dataTransfer.items).forEach(async (item) => await pasteFile(item, editor, undefined, insertParentId, insertIndex));

		draggingData = undefined;
		fakeHighlightOfNotYetSelectedLayerBeingDragged = undefined;
		dragInPanel = false;
		stopAutoScroll();
	}

	// O(listed layers), so it's skipped when only the layers' details change
	function rebuildLayerHierarchy(layerStructure: LayerStructureEntry[]) {
		if (layerStructure === layersBuiltFrom) return;
		layersBuiltFrom = layerStructure;

		// Track the editing state by flat list index, not layer ID, since a layer can appear at multiple positions
		const editingIndex = layers.findIndex((layer: LayerListingInfo) => layer.editingName);

		// Clear the layer hierarchy before rebuilding it
		layers = [];
		let top = 0;

		// Build the new layer hierarchy
		const recurse = (children: LayerStructureEntry[], depth: number, parentId: bigint | undefined, parentPath: bigint[], parentsVisible: boolean, parentsUnlocked: boolean) => {
			children.forEach((item, index) => {
				const treePath = [...parentPath, item.layerId];
				const height = LAYER_ROW_HEIGHT + item.chainRows * CHAIN_NODE_ROW_HEIGHT;

				layers.push({
					id: item.layerId,
					folderIndex: index,
					bottomLayer: index === children.length - 1,
					editingName: editingIndex === layers.length,
					entry: undefined,
					depth,
					parentId,
					parentsVisible,
					parentsUnlocked,
					childrenPresent: item.childrenPresent,
					expanded: item.childrenPresent && item.children.length > 0,
					selected: item.selected,
					ancestorOfSelected: item.descendantSelected,
					treePath,
					top,
					height,
				});
				top += height;

				if (item.children.length >= 1) recurse(item.children, depth + 1, item.layerId, treePath, parentsVisible && item.visible, parentsUnlocked && item.unlocked);
			});
		};
		recurse(layerStructure, 1, undefined, [], true, true);
		listHeight = top;

		updateRenderedRows();
	}
</script>

<LayoutCol class="layers" on:dragleave={fileDragLeave}>
	<LayoutRow class="control-bar" scrollableX={true}>
		<WidgetLayout layout={layersPanelControlBarLeftLayout} layoutTarget="LayersPanelControlLeftBar" />
	</LayoutRow>
	<LayoutRow class="list-area" classes={{ "drag-ongoing": Boolean(internalDragState?.active && draggingData) }} scrollableY={true} bind:this={listArea} on:scroll={listAreaScroll}>
		<LayoutCol
			class="list"
			styles={{ cursor: layerToClipUponClick && layerToClipAltKeyPressed && layerToClipUponClick.entry?.clippable ? "alias" : "auto" }}
			data-layer-panel
			bind:this={list}
			on:click={() => deselectAllLayers()}
			on:dragover={fileDragOver}
			on:drop={fileDrop}
		>
			<!-- Stand-ins as tall as the rows left unrendered above and below the rendered ones -->
			<div class="unrendered-rows" style:height={`${layers.at(firstRendered)?.top || 0}px`}></div>
			<!-- Keyed by position in the whole list, so a row scrolling into view gets new elements that measure their chain width -->
			{#each layers.slice(firstRendered, endRendered) as listing, renderedIndex (firstRendered + renderedIndex)}
				{@const index = firstRendered + renderedIndex}
				{#if !listing.entry}
					<!-- Held open until the backend sends the details its row needs -->
					<div class="layer-placeholder" style:height={`${listing.height}px`}></div>
				{:else}
					{@const entry = listing.entry}
					{@const selected = fakeHighlightOfNotYetSelectedLayerBeingDragged !== undefined ? fakeHighlightOfNotYetSelectedLayerBeingDragged === listing.id : entry.selected}
					{@const slots = chainSlots(entry)}
					{@const inlineSlots = entry.chainExpanded && entry.chainNodes.length > 0 ? slots.filter((slot) => slot.kind === "insertion") : slots}
					{@const ellipsis = chainEllipsis(inlineSlots, chainWidths[index])}
					<LayoutRow
						class="layer"
						classes={{
							selected,
							"ancestor-of-selected": listing.ancestorOfSelected,
							"descendant-of-selected": entry.descendantOfSelected,
							"selected-but-not-in-selected-network": selected && !entry.inSelectedNetwork,
							"insert-folder": (draggingData?.highlightFolder || false) && draggingData?.highlightFolderIndex === index,
						}}
						styles={{ "--layer-indent-levels": `${listing.depth - 1}` }}
						data-layer
						data-index={index}
						on:pointerdown={(e) => layerPointerDown(e, listing)}
						on:click={(e) => selectLayerWithModifiers(e, listing)}
						on:dblclick={(e) => chainIconsDoubleClick(e, listing)}
						on:pointerenter={() => hoverRow(index)}
						on:pointerleave={() => (hoveredRowIndex = undefined)}
					>
						<IconButton
							class="status-toggle"
							classes={{ inherited: !listing.parentsVisible }}
							action={(e) => (toggleNodeVisibilityLayerPanel(listing.id), e?.stopPropagation())}
							size={24}
							icon={entry.visible ? "EyeVisible" : "EyeHidden"}
							hoverIcon={entry.visible ? "EyeHide" : "EyeShow"}
							tooltipLabel={entry.visible ? "Hide" : "Show"}
							tooltipDescription={!listing.parentsVisible ? "A parent of this layer is hidden and that status is being inherited." : ""}
							data-drag-toggle-group="layer-visibility"
							data-drag-toggle-state={entry.visible ? "visible" : "hidden"}
						/>
						{#if entry.childrenAllowed || listing.childrenPresent}
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
							></button>
						{:else}
							<div class="expand-arrow-none"></div>
						{/if}
						{#if entry.clipped}
							<IconLabel
								icon="Clipped"
								class="clipped-arrow"
								tooltipLabel="Layer Clipped"
								tooltipDescription="Clipping mask is active. To release it, target the bottom border of the layer and perform the shortcut shown."
								tooltipShortcut={$tooltip.altClickShortcut}
							/>
						{/if}
						<div class="thumbnail">
							{#if $nodeGraph.thumbnails.has(listing.id)}
								{@html $nodeGraph.thumbnails.get(listing.id)}
							{/if}
						</div>
						<div class="layer-name-and-chain" class:editing={listing.editingName} style:--chain-width={`${chainFullWidth(inlineSlots.length, slots.length > 0)}px`}>
							<LayoutRow class="layer-name" on:dblclick={() => onEditLayerName(listing)}>
								<input
									class="name-input"
									data-layer-name-input
									type="text"
									value={entry.alias}
									placeholder={entry.implementationName}
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
							{#if !entry.unlocked || !listing.parentsUnlocked}
								<IconButton
									class="status-toggle layer-lock"
									classes={{ inherited: !listing.parentsUnlocked }}
									action={(e) => (toggleLayerLock(listing.id), e?.stopPropagation())}
									size={24}
									icon={entry.unlocked ? "PadlockUnlocked" : "PadlockLocked"}
									hoverIcon={entry.unlocked ? "PadlockLocked" : "PadlockUnlocked"}
									tooltipLabel={entry.unlocked ? "Lock" : "Unlock"}
									tooltipDescription={!listing.parentsUnlocked ? "A parent of this layer is locked and that status is being inherited." : ""}
									data-drag-toggle-group="layer-lock"
									data-drag-toggle-state={entry.unlocked ? "unlocked" : "locked"}
								/>
							{:else if activeDragToggleGroup === "layer-lock"}
								<IconButton
									class="status-toggle layer-lock drag-toggle-placeholder"
									action={(e) => (toggleLayerLock(listing.id), e?.stopPropagation())}
									size={24}
									icon="PadlockUnlocked"
									data-drag-toggle-group="layer-lock"
									data-drag-toggle-state="unlocked"
								/>
							{/if}
							<LayoutRow class="layer-name-spacer" />
							<div class="layer-chain-icons" data-layer-chain use:observeChainWidth>
								{#if chainFits(inlineSlots.length, slots.length > 0, chainWidths[index])}
									<!-- An expanded chain lists its nodes in rows beneath the layer instead, keeping only a ghost node here -->
									{#each inlineSlots as slot, slotIndex}
										{#if ellipsis && slotIndex === ellipsis.start}
											<LayoutRow
												class="chain-ellipsis"
												classes={{
													selected: ellipsis.nodes.some((node) => node.selected),
													"tool-target": ellipsis.nodes.some((node) => node.toolTargetDescription),
													"edit-in-progress": ellipsis.nodes.some((node) => node.editInProgress),
												}}
												tooltipLabel={`${ellipsis.nodes.length} More Nodes`}
												tooltipDescription={ellipsis.nodes.map((node) => node.name).join(", ")}
												data-chain-gap-before={ellipsis.gapBefore}
												data-chain-gap-after={ellipsis.gapAfter}
											>
												<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 12 12">
													<circle cx="2" cy="11" r="1" />
													<circle cx="6" cy="11" r="1" />
													<circle cx="10" cy="11" r="1" />
												</svg>
											</LayoutRow>
										{/if}
										{#if !ellipsis || slotIndex < ellipsis.start || slotIndex >= ellipsis.end}
											{#if slot.kind === "insertion"}
												<IconLabel
													icon={slot.insertion.icon}
													class="tool-target-insertion"
													tooltipLabel={slot.insertion.name}
													tooltipDescription={slot.insertion.description}
												/>
											{:else}
												{@const chainNode = slot.node}
												<IconButton
													icon={chainNode.icon}
													size={24}
													classes={{
														selected: chainNode.selected,
														hidden: !chainNode.visible,
														"tool-target": Boolean(chainNode.toolTargetDescription),
														"edit-in-progress": chainNode.editInProgress,
													}}
													action={(e) => chainNodeClick(e, listing, chainNode.id)}
													tooltipLabel={chainNodeTooltipLabel(chainNode)}
													tooltipDescription={chainNode.toolTargetDescription}
													data-chain-node={slot.nodeIndex}
													data-chain-gap-before={slot.reorderablePosition}
													data-chain-gap-after={slot.reorderablePosition === undefined ? undefined : slot.reorderablePosition + 1}
												/>
											{/if}
										{/if}
									{/each}
									<button
										class="chain-expand-button"
										class:with-arrow={slots.length > 0}
										class:expandable={entry.chainNodes.length > 0}
										data-chain-expandable={entry.chainNodes.length > 0 ? "" : undefined}
										class:expanded={entry.chainExpanded && entry.chainNodes.length > 0}
										data-tooltip-label={entry.chainNodes.length === 0 ? undefined : entry.chainExpanded ? "Collapse Chain" : "Expand Chain"}
										data-tooltip-description={entry.chainNodes.length === 0
											? undefined
											: entry.chainExpanded
												? "Stop listing this layer's chain one node per row."
												: "List this layer's chain one node per row, beneath the layer."}
										on:click={(e) => {
											// Without a chain to expand, a click selects the layer as it does elsewhere in the row
											if (entry.chainNodes.length === 0) return;
											e.stopPropagation();
											editor.toggleLayerChainExpanded(listing.id);
										}}
									>
										<IconLabel
											icon={entry.iconName}
											class="layer-type-icon"
											classes={{ hidden: !entry.visible, "tool-target": Boolean(entry.toolTargetDescription), "edit-in-progress": entry.editInProgress }}
											tooltipLabel={entry.implementationName}
											tooltipDescription={entry.toolTargetDescription}
											data-layer-icon
										/>
									</button>
								{/if}
							</div>
						</div>
						{#if chainDropRowIndex === index && chainInsertMarkerLeft !== undefined}
							<div class="chain-insert-mark" style:left={`${chainInsertMarkerLeft}px`}></div>
						{/if}
					</LayoutRow>
					{#if entry.chainExpanded}
						<!-- Downstream first, so the node feeding the layer sits right beneath it -->
						{#each slots.flatMap((slot) => (slot.kind === "node" ? [slot] : [])).reverse() as slot}
							{@const chainNode = slot.node}
							{@const editingName = editingChainNodeName?.rowIndex === index && editingChainNodeName.nodeId === chainNode.id}
							<LayoutRow
								class="chain-node-row"
								classes={{ selected: chainNode.selected }}
								styles={{ "--layer-indent-levels": `${listing.depth - 1}` }}
								data-chain-row-of={index}
								data-chain-row-node={slot.nodeIndex}
								data-chain-gap-before={slot.reorderablePosition}
								data-chain-gap-after={slot.reorderablePosition === undefined ? undefined : slot.reorderablePosition + 1}
								on:pointerdown={(e) => {
									if (e.button === 0 && draggable) chainNodePointerDown(e, listing, chainNode);
								}}
								on:click={(e) => chainNodeClick(e, listing, chainNode.id)}
								on:dblclick={(e) => {
									// Only the node's icon opens it in the graph
									if (e.target instanceof Element && e.target.closest("[data-chain-node-row-icon]")) editor.showNodeInGraph(chainNode.id);
								}}
								on:pointerenter={() => hoverRow(index)}
								on:pointerleave={() => (hoveredRowIndex = undefined)}
							>
								<IconButton
									class="chain-node-visibility"
									action={(e) => (toggleNodeVisibilityLayerPanel(chainNode.id), e?.stopPropagation())}
									size={24}
									icon={chainNode.visible ? "EyeVisible" : "EyeHidden"}
									hoverIcon={chainNode.visible ? "EyeHide" : "EyeShow"}
									tooltipLabel={chainNode.visible ? "Hide" : "Show"}
									data-drag-toggle-group="layer-visibility"
									data-drag-toggle-state={chainNode.visible ? "visible" : "hidden"}
								/>
								<LayoutRow class="chain-node-name" on:dblclick={() => onEditChainNodeName(index, chainNode.id)}>
									<input
										class="name-input"
										data-chain-node-name-input
										type="text"
										value={chainNode.name}
										placeholder={chainNode.implementationName}
										disabled={!editingName}
										on:blur={() => onEditChainNodeNameDeselect(chainNode)}
										on:keydown={(e) => {
											if (e.key === "Escape") onEditChainNodeNameDeselect(chainNode);
											else if (e.key === "Enter") onEditChainNodeNameChange(chainNode.id, e);
										}}
										on:change={(e) => onEditChainNodeNameChange(chainNode.id, e)}
									/>
								</LayoutRow>
								<IconLabel
									icon={chainNode.icon}
									class="chain-node-icon"
									classes={{ "tool-target": Boolean(chainNode.toolTargetDescription), "edit-in-progress": chainNode.editInProgress }}
									tooltipLabel={chainNode.implementationName}
									tooltipDescription={chainNode.toolTargetDescription}
									data-chain-node-row-icon
								/>
								{#if chainDropRowIndex === index && chainRowInsertMarker?.nodeIndex === slot.nodeIndex}
									<div class="chain-row-insert-mark" class:bottom={chainRowInsertMarker.edge === "bottom"}></div>
								{/if}
							</LayoutRow>
						{/each}
					{/if}
				{/if}
			{/each}
			<div class="unrendered-rows" style:height={`${listHeight - (layers.at(endRendered)?.top ?? listHeight)}px`}></div>
		</LayoutCol>
		{#if draggingData && !draggingData.highlightFolder && dragInPanel}
			<div class="insert-mark" style:left={`${4 + 24 + draggingData.insertDepth * 16}px`} style:top={`${draggingData.markerHeight}px`}></div>
		{/if}
	</LayoutRow>
	<LayoutRow
		class="bottom-bar"
		classes={{ "layer-drag-active": Boolean(internalDragState?.active), "chain-node-drag-active": chainDragActive }}
		scrollableX={true}
		bind:this={bottomBar}
		data-layer-bottom-bar
	>
		<WidgetLayout layout={layersPanelBottomBarLeftLayout} layoutTarget="LayersPanelBottomLeftBar" />
		<WidgetLayout layout={layersPanelBottomBarRightLayout} layoutTarget="LayersPanelBottomRightBar" />
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
				// Widgets squished to their minimums overflow into the bar so it can scroll
				overflow: visible;

				> :is(.dropdown-input, .number-input) {
					min-width: 64px;
				}

				// The blend mode is sized for its longest name, so it gives up that spare room before the fields lose any of theirs
				> .dropdown-input {
					flex-shrink: 10000;
				}
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
			// The browser would otherwise shift the scroll position to make up for the padding that stands in for unrendered rows changing as they're rendered
			overflow-anchor: none;

			&.drag-ongoing .layer {
				pointer-events: none;
			}

			.unrendered-rows,
			.layer-placeholder {
				flex: 0 0 auto;
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

				// Spanning the insertion line beside the layer (past the visibility column and the indent), plus its own thickness outward so it clears the icons at both ends
				&.insert-folder::after {
					--outline-thickness: 3px;
					content: "";
					position: absolute;
					inset: 0;
					left: calc(24px + var(--layer-indent-levels) * 16px - var(--outline-thickness));
					right: calc(-1 * var(--outline-thickness));
					border: var(--outline-thickness) solid var(--color-e-nearwhite);
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

				// The name (and lock) and the chain split the rest of the row evenly, except that either gives the other whatever share it doesn't need.
				// Between them, the spacer keeps an unrelated-width gap.
				.layer-name-and-chain {
					flex: 1 1 0;
					min-width: 0;
					align-self: stretch;
					display: grid;
					grid-template-columns: minmax(0, max-content) auto minmax(4px, 1fr) minmax(0, var(--chain-width));
					align-items: center;

					// While renaming, the name takes all the room its text needs, which the chain gives up until it disappears
					&.editing {
						display: flex;

						// Never narrower than its text, up to the whole row (less the lock beside it)
						.layer-name {
							flex: 1 0 auto;
							max-width: calc(100% - 2 * 8px);
						}

						&:has(.layer-lock) .layer-name {
							max-width: calc(100% - 2 * 8px - 24px);
						}

						.layer-name-spacer {
							flex: 0 0 4px;
						}

						.layer-chain-icons {
							flex: 0 1 var(--chain-width);
							min-width: 0;
						}
					}
				}

				.layer-lock {
					grid-column: 2;
				}

				// The layer icon, and with a chain, an arrow before it, which open the chain into a row per node beneath the layer, the arrow turning like the one that expands a layer's children
				.chain-expand-button {
					flex: 0 0 auto;
					display: flex;
					align-items: center;
					height: 24px;
					padding: 0;
					margin: 0;
					border: none;
					border-radius: 2px;
					background: none;

					// Grayed out for a chain with only the ghost of a node, which has nothing to expand
					&.with-arrow::before {
						content: "";
						flex: 0 0 auto;
						width: 8px;
						height: 8px;
						margin: 0 2px;
						background: var(--icon-expand-collapse-arrow-disabled);
					}

					&.expandable::before {
						background: var(--icon-expand-collapse-arrow);
					}

					&.expanded::before {
						transform: rotate(90deg);
					}

					&.expandable:hover {
						background: var(--color-5-dullgray);

						&::before {
							background: var(--icon-expand-collapse-arrow-hover);
						}
					}
				}

				// Right-aligned, since its share of the row can be a little wider than the whole slots that fit
				.layer-chain-icons {
					grid-column: 4;
					display: flex;
					align-items: center;
					justify-content: flex-end;
					overflow: hidden;

					.icon-button {
						width: 20px;
					}

					.icon-button.selected,
					.chain-ellipsis.selected {
						background: var(--color-6-lowergray);
						z-index: 1;
					}

					// Stands in for the chain's middle nodes that don't fit, as wide as a chain button, with its dots at the bottom like a typographic ellipsis rather than centered like an icon
					.chain-ellipsis {
						flex: 0 0 auto;
						padding: 6px 4px;
						border-radius: 2px;

						svg {
							width: 12px;
							height: 12px;
							fill: var(--color-e-nearwhite);
						}
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

					// Takes the same 20px width as the chain buttons
					.layer-type-icon {
						margin: 0 2px;
						align-self: center;
					}

					// The node the active tool edits, or the ghost of one it would add, brightened while an edit in progress writes to it
					.tool-target,
					.tool-target-insertion,
					.edit-in-progress {
						outline: 1px solid var(--color-8-uppergray);
						outline-offset: -1px;
						border-radius: 2px;
					}

					.edit-in-progress {
						outline-color: var(--color-e-nearwhite);
					}

					// Outside the bare 16px layer icon, rather than inside a 20px button
					.layer-type-icon.tool-target,
					.layer-type-icon.edit-in-progress {
						outline-offset: 1px;
					}

					// Pads the 16px icon to the 20px by 24px of the chain buttons
					.tool-target-insertion {
						padding: 4px 2px;

						svg {
							opacity: 0.5;
						}
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

				// Pushes the chain icons to the row's end
				.layer-name-spacer {
					grid-column: 3;
					align-self: stretch;
				}

				// Only as wide as its text, so the lock icon follows right after it, but shrinks to an ellipsis when space runs out
				.layer-name {
					grid-column: 1;
					margin: 0 8px;
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

			// The layer and node names, which look like plain text until renamed, and are only as wide as their text either way
			.name-input {
				field-sizing: content;
				max-width: 100%;
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

				// The padding grows leftward so the text stays where it was before renaming
				&:focus {
					background: var(--color-1-nearblack);
					padding: 0 4px;
					margin-left: -4px;
					max-width: calc(100% + 4px);

					&::placeholder {
						opacity: 0.5;
					}
				}

				&::placeholder {
					opacity: 1;
					color: inherit;
				}
			}

			// A node of an expanded chain, in a skinnier row beneath its layer, with its name and icon in line with the layer's
			.chain-node-row {
				// The eye's start, lining its right edge up with the thumbnail's: past the visibility column, indent, expand arrow, and thumbnail, less its width
				--chain-node-row-inset: calc(24px + var(--layer-indent-levels) * 16px + 16px + 4px + 36px - 20px);
				flex: 0 0 auto;
				align-items: center;
				position: relative;
				height: 24px;
				margin: 0 4px;
				border-bottom: 1px solid var(--color-2-mildblack);
				border-radius: 2px;

				&.selected {
					background: var(--color-4-dimgray);
				}

				.chain-node-visibility {
					width: 20px;
					margin-left: var(--chain-node-row-inset);
				}

				// In line with the layer name, and only as wide as its text, so double-clicking beside it doesn't rename
				.chain-node-name {
					flex: 0 1 auto;
					min-width: 0;
					margin: 0 8px;
				}

				// At the row's end, past a margin like the layer icon's
				.chain-node-icon {
					margin-left: auto;
					margin-right: 2px;

					// Outlined like the layer icon when the active tool edits it
					&.tool-target,
					&.edit-in-progress {
						outline: 1px solid var(--color-8-uppergray);
						outline-offset: 1px;
						border-radius: 2px;
					}

					&.edit-in-progress {
						outline-color: var(--color-e-nearwhite);
					}
				}

				// The horizontal counterpart of the marker in the layer's chain, centered on the border between rows, from the eye onward
				.chain-row-insert-mark {
					position: absolute;
					left: var(--chain-node-row-inset);
					right: 0;
					top: -3px;
					height: 5px;
					background: var(--color-e-nearwhite);
					z-index: 1;
					pointer-events: none;

					&.bottom {
						top: auto;
						bottom: -3px;
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

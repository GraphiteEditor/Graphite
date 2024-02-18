<script lang="ts">
	import { getContext, onMount, tick } from "svelte";
	import { fade } from "svelte/transition";

	import { FADE_TRANSITION } from "@graphite/consts";
	import type { NodeGraphState } from "@graphite/state-providers/node-graph";
	import type { IconName } from "@graphite/utility-functions/icons";
	import type { Editor } from "@graphite/wasm-communication/editor";
	import type { FrontendNodeLink, FrontendNodeType, FrontendNode, FrontendGraphInput, FrontendGraphOutput } from "@graphite/wasm-communication/messages";

	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import IconButton from "@graphite/components/widgets/buttons/IconButton.svelte";
	import TextButton from "@graphite/components/widgets/buttons/TextButton.svelte";
	import TextInput from "@graphite/components/widgets/inputs/TextInput.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";

	const WHEEL_RATE = (1 / 600) * 3;
	const GRID_COLLAPSE_SPACING = 10;
	const GRID_SIZE = 24;
	const ADD_NODE_MENU_WIDTH = 180;
	const ADD_NODE_MENU_HEIGHT = 200;

	const editor = getContext<Editor>("editor");
	const nodeGraph = getContext<NodeGraphState>("nodeGraph");

	type LinkPath = { pathString: string; dataType: string; thick: boolean };

	let graph: HTMLDivElement | undefined;
	let nodesContainer: HTMLDivElement | undefined;
	let nodeSearchInput: TextInput | undefined;
	// TODO: MEMORY LEAK: Items never get removed from this array, so find a way to deal with garbage collection
	let layerNameLabelWidths: Record<string, number> = {};

	let transform = { scale: 1, x: 1200, y: 0 };
	let panning = false;
	let draggingNodes: { startX: number; startY: number; roundX: number; roundY: number } | undefined = undefined;
	type Box = { startX: number; startY: number; endX: number; endY: number };
	let boxSelection: Box | undefined = undefined;
	let previousSelection: bigint[] = [];
	let selectIfNotDragged: undefined | bigint = undefined;
	let linkInProgressFromConnector: SVGSVGElement | undefined = undefined;
	let linkInProgressToConnector: SVGSVGElement | DOMRect | undefined = undefined;
	// TODO: Using this not-complete code, or another better approach, make it so the dragged in-progress connector correctly handles showing/hiding the SVG shape of the connector caps
	// let linkInProgressFromLayerTop: bigint | undefined = undefined;
	// let linkInProgressFromLayerBottom: bigint | undefined = undefined;
	let disconnecting: { nodeId: bigint; inputIndex: number; linkIndex: number } | undefined = undefined;
	let nodeLinkPaths: LinkPath[] = [];
	let searchTerm = "";
	let nodeListLocation: { x: number; y: number } | undefined = undefined;

	let inputs: SVGSVGElement[][] = [];
	let outputs: SVGSVGElement[][] = [];
	let nodeElements: HTMLDivElement[] = [];

	$: watchNodes($nodeGraph.nodes);

	$: gridSpacing = calculateGridSpacing(transform.scale);
	$: dotRadius = 1 + Math.floor(transform.scale - 0.5 + 0.001) / 2;
	$: nodeCategories = buildNodeCategories($nodeGraph.nodeTypes, searchTerm);
	$: nodeListX = ((nodeListLocation?.x || 0) * GRID_SIZE + transform.x) * transform.scale;
	$: nodeListY = ((nodeListLocation?.y || 0) * GRID_SIZE + transform.y) * transform.scale;

	let appearAboveMouse = false;
	let appearRightOfMouse = false;

	$: (() => {
		const bounds = graph?.getBoundingClientRect();
		if (!bounds) return;
		const { width, height } = bounds;

		appearRightOfMouse = nodeListX > width - ADD_NODE_MENU_WIDTH / 2;
		appearAboveMouse = nodeListY > height - ADD_NODE_MENU_HEIGHT / 2;
	})();

	$: linkPathInProgress = createLinkPathInProgress(linkInProgressFromConnector, linkInProgressToConnector);
	$: linkPaths = createLinkPaths(linkPathInProgress, nodeLinkPaths);

	function calculateGridSpacing(scale: number): number {
		const dense = scale * GRID_SIZE;
		let sparse = dense;

		while (sparse > 0 && sparse < GRID_COLLAPSE_SPACING) {
			sparse *= 2;
		}

		return sparse;
	}

	type NodeCategoryDetails = {
		nodes: FrontendNodeType[];
		open: boolean;
	};

	function buildNodeCategories(nodeTypes: FrontendNodeType[], searchTerm: string): [string, NodeCategoryDetails][] {
		const categories = new Map<string, NodeCategoryDetails>();

		nodeTypes.forEach((node) => {
			const nameIncludesSearchTerm = node.name.toLowerCase().includes(searchTerm.toLowerCase());

			if (searchTerm.length > 0 && !nameIncludesSearchTerm && !node.category.toLowerCase().includes(searchTerm.toLowerCase())) {
				return;
			}

			const category = categories.get(node.category);
			let open = nameIncludesSearchTerm;
			if (searchTerm.length === 0) {
				open = false;
			}

			if (category) {
				category.open = open;
				category.nodes.push(node);
			} else
				categories.set(node.category, {
					open: open,
					nodes: [node],
				});
		});

		return Array.from(categories);
	}

	function createLinkPathInProgress(linkInProgressFromConnector?: SVGSVGElement, linkInProgressToConnector?: SVGSVGElement | DOMRect): LinkPath | undefined {
		if (linkInProgressFromConnector && linkInProgressToConnector && nodesContainer) {
			const from = connectorToNodeIndex(linkInProgressFromConnector);
			const to = linkInProgressToConnector instanceof SVGSVGElement ? connectorToNodeIndex(linkInProgressToConnector) : undefined;

			const linkStart = $nodeGraph.nodes.find((n) => n.id === from?.nodeId)?.isLayer || false;
			const linkEnd = ($nodeGraph.nodes.find((n) => n.id === to?.nodeId)?.isLayer && to?.index !== 0) || false;
			return createWirePath(linkInProgressFromConnector, linkInProgressToConnector, linkStart, linkEnd);
		}
		return undefined;
	}

	function createLinkPaths(linkPathInProgress: LinkPath | undefined, nodeLinkPaths: LinkPath[]): LinkPath[] {
		const maybeLinkPathInProgress = linkPathInProgress ? [linkPathInProgress] : [];
		return [...maybeLinkPathInProgress, ...nodeLinkPaths];
	}

	async function watchNodes(nodes: FrontendNode[]) {
		nodes.forEach((_, index) => {
			if (!inputs[index]) inputs[index] = [];
			if (!outputs[index]) outputs[index] = [];
		});

		await refreshLinks();
	}

	function resolveLink(link: FrontendNodeLink): { nodeOutput: SVGSVGElement | undefined; nodeInput: SVGSVGElement | undefined } {
		const outputIndex = Number(link.linkStartOutputIndex);
		const inputIndex = Number(link.linkEndInputIndex);

		const nodeOutputConnectors = outputs[$nodeGraph.nodes.findIndex((n) => n.id === link.linkStart)];
		const nodeInputConnectors = inputs[$nodeGraph.nodes.findIndex((n) => n.id === link.linkEnd)] || undefined;

		const nodeOutput = nodeOutputConnectors?.[outputIndex] as SVGSVGElement | undefined;
		const nodeInput = nodeInputConnectors?.[inputIndex] as SVGSVGElement | undefined;
		return { nodeOutput, nodeInput };
	}

	async function refreshLinks() {
		await tick();

		const links = $nodeGraph.links;
		nodeLinkPaths = links.flatMap((link, index) => {
			const { nodeInput, nodeOutput } = resolveLink(link);
			if (!nodeInput || !nodeOutput) return [];
			if (disconnecting?.linkIndex === index) return [];

			const linkStart = $nodeGraph.nodes.find((n) => n.id === link.linkStart)?.isLayer || false;
			const linkEnd = ($nodeGraph.nodes.find((n) => n.id === link.linkEnd)?.isLayer && link.linkEndInputIndex !== 0n) || false;

			return [createWirePath(nodeOutput, nodeInput.getBoundingClientRect(), linkStart, linkEnd)];
		});
	}

	onMount(refreshLinks);

	function nodeIcon(nodeName: string): IconName {
		const iconMap: Record<string, IconName> = {
			Output: "NodeOutput",
		};
		return iconMap[nodeName] || "NodeNodes";
	}

	function buildWirePathLocations(outputBounds: DOMRect, inputBounds: DOMRect, verticalOut: boolean, verticalIn: boolean): { x: number; y: number }[] {
		if (!nodesContainer) return [];

		const VERTICAL_LINK_OVERLAP_ON_SHAPED_CAP = 1;

		const containerBounds = nodesContainer.getBoundingClientRect();

		const outX = verticalOut ? outputBounds.x + outputBounds.width / 2 : outputBounds.x + outputBounds.width - 1;
		const outY = verticalOut ? outputBounds.y + VERTICAL_LINK_OVERLAP_ON_SHAPED_CAP : outputBounds.y + outputBounds.height / 2;
		const outConnectorX = (outX - containerBounds.x) / transform.scale;
		const outConnectorY = (outY - containerBounds.y) / transform.scale;

		const inX = verticalIn ? inputBounds.x + inputBounds.width / 2 : inputBounds.x + 1;
		const inY = verticalIn ? inputBounds.y + inputBounds.height - VERTICAL_LINK_OVERLAP_ON_SHAPED_CAP : inputBounds.y + inputBounds.height / 2;
		const inConnectorX = (inX - containerBounds.x) / transform.scale;
		const inConnectorY = (inY - containerBounds.y) / transform.scale;
		const horizontalGap = Math.abs(outConnectorX - inConnectorX);
		const verticalGap = Math.abs(outConnectorY - inConnectorY);

		// TODO: Finish this commented out code replacement for the code below it based on this diagram: <https://files.keavon.com/-/InsubstantialElegantQueenant/capture.png>
		// // Straight: stacking lines which are always straight, or a straight horizontal link between two aligned nodes
		// if ((verticalOut && verticalIn) || (!verticalOut && !verticalIn && verticalGap === 0)) {
		// 	return [
		// 		{ x: outConnectorX, y: outConnectorY },
		// 		{ x: inConnectorX, y: inConnectorY },
		// 	];
		// }

		// // L-shape bend
		// if (verticalOut !== verticalIn) {
		// }

		const curveLength = 24;
		const curveFalloffRate = curveLength * Math.PI * 2;

		const horizontalCurveAmount = -(2 ** ((-10 * horizontalGap) / curveFalloffRate)) + 1;
		const verticalCurveAmount = -(2 ** ((-10 * verticalGap) / curveFalloffRate)) + 1;
		const horizontalCurve = horizontalCurveAmount * curveLength;
		const verticalCurve = verticalCurveAmount * curveLength;

		return [
			{ x: outConnectorX, y: outConnectorY },
			{ x: verticalOut ? outConnectorX : outConnectorX + horizontalCurve, y: verticalOut ? outConnectorY - verticalCurve : outConnectorY },
			{ x: verticalIn ? inConnectorX : inConnectorX - horizontalCurve, y: verticalIn ? inConnectorY + verticalCurve : inConnectorY },
			{ x: inConnectorX, y: inConnectorY },
		];
	}

	function buildWirePathString(outputBounds: DOMRect, inputBounds: DOMRect, verticalOut: boolean, verticalIn: boolean): string {
		const locations = buildWirePathLocations(outputBounds, inputBounds, verticalOut, verticalIn);
		if (locations.length === 0) return "[error]";
		const SMOOTHING = 0.5;
		const delta01 = { x: (locations[1].x - locations[0].x) * SMOOTHING, y: (locations[1].y - locations[0].y) * SMOOTHING };
		const delta23 = { x: (locations[3].x - locations[2].x) * SMOOTHING, y: (locations[3].y - locations[2].y) * SMOOTHING };
		return `
			M${locations[0].x},${locations[0].y}
			L${locations[1].x},${locations[1].y}
			C${locations[1].x + delta01.x},${locations[1].y + delta01.y}
			${locations[2].x - delta23.x},${locations[2].y - delta23.y}
			${locations[2].x},${locations[2].y}
			L${locations[3].x},${locations[3].y}
			`
			.split("\n")
			.map((line) => line.trim())
			.join(" ");
	}

	function createWirePath(outputPort: SVGSVGElement, inputPort: SVGSVGElement | DOMRect, verticalOut: boolean, verticalIn: boolean): LinkPath {
		const inputPortRect = inputPort instanceof DOMRect ? inputPort : inputPort.getBoundingClientRect();

		const pathString = buildWirePathString(outputPort.getBoundingClientRect(), inputPortRect, verticalOut, verticalIn);
		const dataType = outputPort.getAttribute("data-datatype") || "general";

		return { pathString, dataType, thick: verticalIn && verticalOut };
	}

	function scroll(e: WheelEvent) {
		const [scrollX, scrollY] = [e.deltaX, e.deltaY];

		// If zoom with scroll is enabled: horizontal pan with Ctrl, vertical pan with Shift
		const zoomWithScroll = $nodeGraph.zoomWithScroll;
		const zoom = zoomWithScroll ? !e.ctrlKey && !e.shiftKey : e.ctrlKey;
		const horizontalPan = zoomWithScroll ? e.ctrlKey : !e.ctrlKey && e.shiftKey;

		// Prevent the web page from being zoomed
		if (e.ctrlKey) e.preventDefault();

		// Always pan horizontally in response to a horizontal scroll wheel movement
		transform.x -= scrollX / transform.scale;

		// Zoom
		if (zoom) {
			let zoomFactor = 1 + Math.abs(scrollY) * WHEEL_RATE;
			if (scrollY > 0) zoomFactor = 1 / zoomFactor;

			const bounds = graph?.getBoundingClientRect();
			if (!bounds) return;
			const { x, y, width, height } = bounds;

			transform.scale *= zoomFactor;

			const newViewportX = width / zoomFactor;
			const newViewportY = height / zoomFactor;

			const deltaSizeX = width - newViewportX;
			const deltaSizeY = height - newViewportY;

			const deltaX = deltaSizeX * ((e.x - x) / width);
			const deltaY = deltaSizeY * ((e.y - y) / height);

			transform.x -= (deltaX / transform.scale) * zoomFactor;
			transform.y -= (deltaY / transform.scale) * zoomFactor;

			return;
		}

		// Pan
		if (horizontalPan) {
			transform.x -= scrollY / transform.scale;
		} else {
			transform.y -= scrollY / transform.scale;
		}
	}

	function keydown(e: KeyboardEvent) {
		if (e.key.toLowerCase() === "escape") {
			nodeListLocation = undefined;
			document.removeEventListener("keydown", keydown);
			linkInProgressFromConnector = undefined;
			// linkInProgressFromLayerTop = undefined;
			// linkInProgressFromLayerBottom = undefined;
		}
	}

	function loadNodeList(e: PointerEvent, graphBounds: DOMRect) {
		nodeListLocation = {
			x: Math.round(((e.clientX - graphBounds.x) / transform.scale - transform.x) / GRID_SIZE),
			y: Math.round(((e.clientY - graphBounds.y) / transform.scale - transform.y) / GRID_SIZE),
		};

		// Find actual relevant child and focus it (setTimeout is required to actually focus the input element)
		setTimeout(() => nodeSearchInput?.focus(), 0);

		document.addEventListener("keydown", keydown);
	}

	// TODO: Move the event listener from the graph to the window so dragging outside the graph area (or even the whole browser window) works
	function pointerDown(e: PointerEvent) {
		const [lmb, rmb] = [e.button === 0, e.button === 2];

		const nodeError = (e.target as SVGSVGElement).closest("[data-node-error]") as HTMLElement;
		if (nodeError && lmb) return;
		const port = (e.target as SVGSVGElement).closest("[data-port]") as SVGSVGElement;
		const node = (e.target as HTMLElement).closest("[data-node]") as HTMLElement | undefined;
		const nodeIdString = node?.getAttribute("data-node") || undefined;
		const nodeId = nodeIdString ? BigInt(nodeIdString) : undefined;
		const nodeList = (e.target as HTMLElement).closest("[data-node-list]") as HTMLElement | undefined;

		// Create the add node popup on right click, then exit
		if (rmb) {
			const graphBounds = graph?.getBoundingClientRect();
			if (!graphBounds) return;
			loadNodeList(e, graphBounds);
			return;
		}

		// If the user is clicking on the add nodes list, exit here
		if (lmb && nodeList) return;

		// Since the user is clicking elsewhere in the graph, ensure the add nodes list is closed
		if (lmb) {
			nodeListLocation = undefined;
			linkInProgressFromConnector = undefined;
			// linkInProgressFromLayerTop = undefined;
			// linkInProgressFromLayerBottom = undefined;
		}

		// Alt-click sets the clicked node as previewed
		if (lmb && e.altKey && nodeId !== undefined) {
			editor.instance.togglePreview(nodeId);
		}

		// Clicked on a port dot
		if (lmb && port && node) {
			const isOutput = Boolean(port.getAttribute("data-port") === "output");
			const frontendNode = (nodeId !== undefined && $nodeGraph.nodes.find((n) => n.id === nodeId)) || undefined;

			// Output: Begin dragging out a new link
			if (isOutput) {
				// Disallow creating additional vertical output links from an already-connected layer
				if (frontendNode?.isLayer && frontendNode.primaryOutput?.connected !== undefined) return;

				linkInProgressFromConnector = port;
				// // Since we are just beginning to drag out a link from the top, we know the in-progress link exists from this layer's top and has no connection to any other layer bottom yet
				// linkInProgressFromLayerTop = nodeId !== undefined && frontendNode?.isLayer ? nodeId : undefined;
				// linkInProgressFromLayerBottom = undefined;
			}
			// Input: Begin moving an existing link
			else {
				const inputNodeInPorts = Array.from(node.querySelectorAll(`[data-port="input"]`));
				const inputNodeConnectionIndexSearch = inputNodeInPorts.indexOf(port);
				// const isLayerBottomConnector = frontendNode?.isLayer && inputNodeConnectionIndexSearch === 1;
				const inputIndex = inputNodeConnectionIndexSearch > -1 ? inputNodeConnectionIndexSearch : undefined;
				if (inputIndex === undefined || nodeId === undefined) return;

				// Set the link to draw from the input that a previous link was on

				const linkIndex = $nodeGraph.links.findIndex((value) => value.linkEnd === nodeId && value.linkEndInputIndex === BigInt(inputIndex));
				if (linkIndex === -1) return;

				const nodeOutputConnectors = nodesContainer?.querySelectorAll(`[data-node="${String($nodeGraph.links[linkIndex].linkStart)}"] [data-port="output"]`) || undefined;
				linkInProgressFromConnector = nodeOutputConnectors?.[Number($nodeGraph.links[linkIndex].linkStartOutputIndex)] as SVGSVGElement | undefined;
				// linkInProgressFromLayerBottom = isLayerBottomConnector ? frontendNode.exposedInputs[0].connected : undefined;

				const nodeInputConnectors = nodesContainer?.querySelectorAll(`[data-node="${String($nodeGraph.links[linkIndex].linkEnd)}"] [data-port="input"]`) || undefined;
				linkInProgressToConnector = nodeInputConnectors?.[Number($nodeGraph.links[linkIndex].linkEndInputIndex)] as SVGSVGElement | undefined;
				// linkInProgressFromLayerTop = undefined;

				disconnecting = { nodeId: nodeId, inputIndex, linkIndex };
				refreshLinks();
			}

			return;
		}

		// Clicked on a node, so we select it
		if (lmb && nodeId !== undefined) {
			let updatedSelected = [...$nodeGraph.selected];
			let modifiedSelected = false;

			// Add to/remove from selection if holding Shift or Ctrl
			if (e.shiftKey || e.ctrlKey) {
				modifiedSelected = true;

				// Remove from selection if already selected
				if (!updatedSelected.includes(nodeId)) updatedSelected.push(nodeId);
				// Add to selection if not already selected
				else updatedSelected.splice(updatedSelected.lastIndexOf(nodeId), 1);
			}
			// Replace selection with a non-selected node
			else if (!updatedSelected.includes(nodeId)) {
				modifiedSelected = true;

				updatedSelected = [nodeId];
			}
			// Replace selection (of multiple nodes including this one) with just this one, but only upon pointer up if the user didn't drag the selected nodes
			else {
				selectIfNotDragged = nodeId;
			}

			// If this node is selected (whether from before or just now), prepare it for dragging
			if (updatedSelected.includes(nodeId)) {
				draggingNodes = { startX: e.x, startY: e.y, roundX: 0, roundY: 0 };
			}

			// Update the selection in the backend if it was modified
			if (modifiedSelected) editor.instance.selectNodes(new BigUint64Array(updatedSelected));

			return;
		}

		// Clicked on the graph background so we box select
		if (lmb) {
			previousSelection = $nodeGraph.selected;
			// Clear current selection
			if (!e.shiftKey) editor.instance.selectNodes(new BigUint64Array(0));

			const graphBounds = graph?.getBoundingClientRect();
			boxSelection = { startX: e.x - (graphBounds?.x || 0), startY: e.y - (graphBounds?.y || 0), endX: e.x - (graphBounds?.x || 0), endY: e.y - (graphBounds?.y || 0) };

			return;
		}

		// LMB clicked on the graph background or MMB clicked anywhere
		panning = true;
	}

	function doubleClick(_e: MouseEvent) {
		// const node = (e.target as HTMLElement).closest("[data-node]") as HTMLElement | undefined;
		// const nodeId = node?.getAttribute("data-node") || undefined;
		// if (nodeId !== undefined) {
		// 	const id = BigInt(nodeId);
		// 	editor.instance.enterNestedNetwork(id);
		// }
	}

	function pointerMove(e: PointerEvent) {
		if (panning) {
			transform.x += e.movementX / transform.scale;
			transform.y += e.movementY / transform.scale;
		} else if (linkInProgressFromConnector && !nodeListLocation) {
			const target = e.target as Element | undefined;
			const dot = (target?.closest(`[data-port="input"]`) || undefined) as SVGSVGElement | undefined;
			if (dot) {
				linkInProgressToConnector = dot;
			} else {
				linkInProgressToConnector = new DOMRect(e.x, e.y);
			}
		} else if (draggingNodes) {
			const deltaX = Math.round((e.x - draggingNodes.startX) / transform.scale / GRID_SIZE);
			const deltaY = Math.round((e.y - draggingNodes.startY) / transform.scale / GRID_SIZE);
			if (draggingNodes.roundX !== deltaX || draggingNodes.roundY !== deltaY) {
				draggingNodes.roundX = deltaX;
				draggingNodes.roundY = deltaY;

				let stop = false;
				const refresh = () => {
					if (!stop) refreshLinks();
					requestAnimationFrame(refresh);
				};
				refresh();
				// const DRAG_SMOOTHING_TIME = 0.1;
				const DRAG_SMOOTHING_TIME = 0; // TODO: Reenable this after fixing the bugs with the wires, see the CSS `transition` attribute todo for other info
				setTimeout(
					() => {
						stop = true;
					},
					DRAG_SMOOTHING_TIME * 1000 + 10,
				);
			}
		} else if (boxSelection) {
			// The mouse button was released but we missed the pointer up event
			if ((e.buttons & 1) === 0) {
				completeBoxSelection();
				boxSelection = undefined;
			} else if ((e.buttons & 2) !== 0) {
				editor.instance.selectNodes(new BigUint64Array(previousSelection));
				boxSelection = undefined;
			} else {
				const graphBounds = graph?.getBoundingClientRect();
				boxSelection.endX = e.x - (graphBounds?.x || 0);
				boxSelection.endY = e.y - (graphBounds?.y || 0);
			}
		}
	}

	function intersetNodeAABB(boxSelection: Box | undefined, nodeIndex: number): boolean {
		const bounds = nodeElements[nodeIndex]?.getBoundingClientRect();
		const graphBounds = graph?.getBoundingClientRect();
		return (
			boxSelection !== undefined &&
			bounds &&
			Math.min(boxSelection.startX, boxSelection.endX) < bounds.right - (graphBounds?.x || 0) &&
			Math.max(boxSelection.startX, boxSelection.endX) > bounds.left - (graphBounds?.x || 0) &&
			Math.min(boxSelection.startY, boxSelection.endY) < bounds.bottom - (graphBounds?.y || 0) &&
			Math.max(boxSelection.startY, boxSelection.endY) > bounds.top - (graphBounds?.y || 0)
		);
	}

	function completeBoxSelection() {
		editor.instance.selectNodes(new BigUint64Array($nodeGraph.selected.concat($nodeGraph.nodes.filter((_, nodeIndex) => intersetNodeAABB(boxSelection, nodeIndex)).map((node) => node.id))));
	}

	function showSelected(selected: bigint[], boxSelect: Box | undefined, node: bigint, nodeIndex: number): boolean {
		return selected.includes(node) || intersetNodeAABB(boxSelect, nodeIndex);
	}

	function toggleLayerVisibility(id: bigint) {
		editor.instance.toggleLayerVisibility(id);
	}

	function connectorToNodeIndex(svg: SVGSVGElement): { nodeId: bigint; index: number } | undefined {
		const node = svg.closest("[data-node]");

		if (!node) return undefined;
		const nodeIdAttribute = node.getAttribute("data-node");
		if (!nodeIdAttribute) return undefined;
		const nodeId = BigInt(nodeIdAttribute);

		const inputPortElements = Array.from(node.querySelectorAll(`[data-port="input"]`));
		const outputPortElements = Array.from(node.querySelectorAll(`[data-port="output"]`));
		const inputNodeConnectionIndexSearch = inputPortElements.includes(svg) ? inputPortElements.indexOf(svg) : outputPortElements.indexOf(svg);
		const index = inputNodeConnectionIndexSearch > -1 ? inputNodeConnectionIndexSearch : undefined;

		if (nodeId !== undefined && index !== undefined) return { nodeId, index };
		else return undefined;
	}

	// Check if this node should be inserted between two other nodes
	function checkInsertBetween() {
		if ($nodeGraph.selected.length !== 1) return;
		const selectedNodeId = $nodeGraph.selected[0];
		const selectedNode = nodesContainer?.querySelector(`[data-node="${String(selectedNodeId)}"]`) || undefined;

		// Check that neither the input or output of the selected node are already connected.
		const notConnected = $nodeGraph.links.findIndex((link) => link.linkStart === selectedNodeId || (link.linkEnd === selectedNodeId && link.linkEndInputIndex === BigInt(0))) === -1;
		const input = selectedNode?.querySelector(`[data-port="input"]`) || undefined;
		const output = selectedNode?.querySelector(`[data-port="output"]`) || undefined;

		// TODO: Make sure inputs are correctly typed
		if (!selectedNode || !notConnected || !input || !output || !nodesContainer) return;

		// Fixes typing for some reason?
		const theNodesContainer = nodesContainer;

		// Find the link that the node has been dragged on top of
		const link = $nodeGraph.links.find((link) => {
			const { nodeInput, nodeOutput } = resolveLink(link);
			if (!nodeInput || !nodeOutput) return false;

			const wireCurveLocations = buildWirePathLocations(nodeOutput.getBoundingClientRect(), nodeInput.getBoundingClientRect(), false, false);

			const selectedNodeBounds = selectedNode.getBoundingClientRect();
			const containerBoundsBounds = theNodesContainer.getBoundingClientRect();

			return editor.instance.rectangleIntersects(
				new Float64Array(wireCurveLocations.map((loc) => loc.x)),
				new Float64Array(wireCurveLocations.map((loc) => loc.y)),
				selectedNodeBounds.top - containerBoundsBounds.y,
				selectedNodeBounds.left - containerBoundsBounds.x,
				selectedNodeBounds.bottom - containerBoundsBounds.y,
				selectedNodeBounds.right - containerBoundsBounds.x,
			);
		});

		// If the node has been dragged on top of the link then connect it into the middle.
		if (link) {
			const isLayer = $nodeGraph.nodes.find((n) => n.id === selectedNodeId)?.isLayer;

			editor.instance.connectNodesByLink(link.linkStart, 0, selectedNodeId, isLayer ? 1 : 0);
			editor.instance.connectNodesByLink(selectedNodeId, 0, link.linkEnd, Number(link.linkEndInputIndex));
			if (!isLayer) editor.instance.shiftNode(selectedNodeId);
		}
	}

	function pointerUp(e: PointerEvent) {
		panning = false;

		const initialDisconnecting = disconnecting;
		if (disconnecting) {
			editor.instance.disconnectNodes(BigInt(disconnecting.nodeId), disconnecting.inputIndex);
		}
		disconnecting = undefined;

		if (linkInProgressToConnector instanceof SVGSVGElement && linkInProgressFromConnector) {
			const from = connectorToNodeIndex(linkInProgressFromConnector);
			const to = connectorToNodeIndex(linkInProgressToConnector);

			if (from !== undefined && to !== undefined) {
				const { nodeId: outputConnectedNodeID, index: outputNodeConnectionIndex } = from;
				const { nodeId: inputConnectedNodeID, index: inputNodeConnectionIndex } = to;
				editor.instance.connectNodesByLink(outputConnectedNodeID, outputNodeConnectionIndex, inputConnectedNodeID, inputNodeConnectionIndex);
			}
		} else if (linkInProgressFromConnector && !initialDisconnecting) {
			// If the add node menu is already open, we don't want to open it again
			if (nodeListLocation) return;

			const graphBounds = graph?.getBoundingClientRect();
			if (!graphBounds) return;

			// Create the node list, which should set nodeListLocation to a valid value
			loadNodeList(e, graphBounds);
			if (!nodeListLocation) return;
			let nodeListLocation2: { x: number; y: number } = nodeListLocation;

			linkInProgressToConnector = new DOMRect(
				(nodeListLocation2.x * GRID_SIZE + transform.x) * transform.scale + graphBounds.x,
				(nodeListLocation2.y * GRID_SIZE + transform.y) * transform.scale + graphBounds.y,
			);

			return;
		} else if (draggingNodes) {
			if (draggingNodes.startX === e.x && draggingNodes.startY === e.y) {
				if (selectIfNotDragged !== undefined && ($nodeGraph.selected.length !== 1 || $nodeGraph.selected[0] !== selectIfNotDragged)) {
					editor.instance.selectNodes(new BigUint64Array([selectIfNotDragged]));
				}
			}

			if ($nodeGraph.selected.length > 0 && (draggingNodes.roundX !== 0 || draggingNodes.roundY !== 0)) editor.instance.moveSelectedNodes(draggingNodes.roundX, draggingNodes.roundY);

			checkInsertBetween();

			draggingNodes = undefined;
			selectIfNotDragged = undefined;
		} else if (boxSelection) {
			completeBoxSelection();
			boxSelection = undefined;
		}

		linkInProgressFromConnector = undefined;
		linkInProgressToConnector = undefined;
	}

	function createNode(nodeType: string) {
		if (!nodeListLocation) return;

		const inputNodeConnectionIndex = 0;
		const inputConnectedNodeID = editor.instance.createNode(nodeType, nodeListLocation.x, nodeListLocation.y - 1);
		nodeListLocation = undefined;

		if (!linkInProgressFromConnector) return;
		const from = connectorToNodeIndex(linkInProgressFromConnector);

		if (from !== undefined) {
			const { nodeId: outputConnectedNodeID, index: outputNodeConnectionIndex } = from;
			editor.instance.connectNodesByLink(outputConnectedNodeID, outputNodeConnectionIndex, inputConnectedNodeID, inputNodeConnectionIndex);
		}

		linkInProgressFromConnector = undefined;
	}

	function nodeBorderMask(nodeWidth: number, primaryInputExists: boolean, parameters: number, primaryOutputExists: boolean, exposedOutputs: number): string {
		const nodeHeight = Math.max(1 + parameters, 1 + exposedOutputs) * 24;

		const boxes: { x: number; y: number; width: number; height: number }[] = [];

		// Primary input
		if (primaryInputExists) boxes.push({ x: -8, y: 4, width: 16, height: 16 });
		// Parameter inputs
		for (let i = 0; i < parameters; i++) boxes.push({ x: -8, y: 4 + (i + 1) * 24, width: 16, height: 16 });

		// Primary output
		if (primaryOutputExists) boxes.push({ x: nodeWidth - 8, y: 4, width: 16, height: 16 });
		// Exposed outputs
		for (let i = 0; i < exposedOutputs; i++) boxes.push({ x: nodeWidth - 8, y: 4 + (i + 1) * 24, width: 16, height: 16 });

		return borderMask(boxes, nodeWidth, nodeHeight);
	}

	function layerBorderMask(nodeWidth: number): string {
		const NODE_HEIGHT = 2 * 24;
		const THUMBNAIL_WIDTH = 72 + 8 * 2;
		const FUDGE_HEIGHT_BEYOND_LAYER_HEIGHT = 2;

		const boxes: { x: number; y: number; width: number; height: number }[] = [];
		// Left input
		boxes.push({ x: -8, y: 16, width: 16, height: 16 });

		// Thumbnail
		boxes.push({ x: 28, y: -FUDGE_HEIGHT_BEYOND_LAYER_HEIGHT, width: THUMBNAIL_WIDTH, height: NODE_HEIGHT + FUDGE_HEIGHT_BEYOND_LAYER_HEIGHT * 2 });

		// Right visibility button
		boxes.push({ x: nodeWidth - 12, y: (NODE_HEIGHT - 24) / 2, width: 24, height: 24 });

		return borderMask(boxes, nodeWidth, NODE_HEIGHT);
	}

	function borderMask(boxes: { x: number; y: number; width: number; height: number }[], nodeWidth: number, nodeHeight: number): string {
		const rectangles = boxes.map((box) => `M${box.x},${box.y} L${box.x + box.width},${box.y} L${box.x + box.width},${box.y + box.height} L${box.x},${box.y + box.height}z`);
		return `M-2,-2 L${nodeWidth + 2},-2 L${nodeWidth + 2},${nodeHeight + 2} L-2,${nodeHeight + 2}z ${rectangles.join(" ")}`;
	}

	function dataTypeTooltip(value: FrontendGraphInput | FrontendGraphOutput): string {
		const dataTypeCapitalized = `${value.dataType[0].toUpperCase()}${value.dataType.slice(1)}`;
		return value.resolvedType ? `Resolved Data: ${value.resolvedType}` : `Unresolved Data: ${dataTypeCapitalized}`;
	}
</script>

<div
	class="graph"
	bind:this={graph}
	on:wheel|nonpassive={scroll}
	on:pointerdown={pointerDown}
	on:pointermove={pointerMove}
	on:pointerup={pointerUp}
	on:dblclick={doubleClick}
	style:--grid-spacing={`${gridSpacing}px`}
	style:--grid-offset-x={`${transform.x * transform.scale}px`}
	style:--grid-offset-y={`${transform.y * transform.scale}px`}
	style:--dot-radius={`${dotRadius}px`}
>
	<!-- Right click menu for adding nodes -->
	{#if nodeListLocation}
		<LayoutCol
			class="node-list"
			data-node-list
			styles={{
				left: `${nodeListX}px`,
				top: `${nodeListY}px`,
				transform: `translate(${appearRightOfMouse ? -100 : 0}%, ${appearAboveMouse ? -100 : 0}%)`,
				width: `${ADD_NODE_MENU_WIDTH}px`,
			}}
		>
			<TextInput placeholder="Search Nodes..." value={searchTerm} on:value={({ detail }) => (searchTerm = detail)} bind:this={nodeSearchInput} />
			<div class="list-nodes" style={`height: ${ADD_NODE_MENU_HEIGHT}px;`} on:wheel|passive|stopPropagation>
				{#each nodeCategories as nodeCategory}
					<details style="display: flex; flex-direction: column;" open={nodeCategory[1].open}>
						<summary>
							<IconLabel icon="DropdownArrow" />
							<TextLabel>{nodeCategory[0]}</TextLabel>
						</summary>
						{#each nodeCategory[1].nodes as nodeType}
							<TextButton label={nodeType.name} action={() => createNode(nodeType.name)} />
						{/each}
					</details>
				{:else}
					<div style="margin-right: 4px;"><TextLabel>No search results</TextLabel></div>
				{/each}
			</div>
		</LayoutCol>
	{/if}
	<!-- Node connection links -->
	<div class="wires" style:transform={`scale(${transform.scale}) translate(${transform.x}px, ${transform.y}px)`} style:transform-origin={`0 0`}>
		<svg>
			{#each linkPaths as { pathString, dataType, thick }}
				<path d={pathString} style:--data-line-width={`${thick ? 8 : 2}px`} style:--data-color={`var(--color-data-${dataType})`} style:--data-color-dim={`var(--color-data-${dataType}-dim)`} />
			{/each}
		</svg>
	</div>
	<!-- Layers and nodes -->
	<div class="layers-and-nodes" style:transform={`scale(${transform.scale}) translate(${transform.x}px, ${transform.y}px)`} style:transform-origin={`0 0`} bind:this={nodesContainer}>
		<!-- Layers -->
		{#each $nodeGraph.nodes.flatMap((node, nodeIndex) => (node.isLayer ? [{ node, nodeIndex }] : [])) as { node, nodeIndex } (nodeIndex)}
			{@const clipPathId = String(Math.random()).substring(2)}
			{@const stackDataInput = node.exposedInputs[0]}
			{@const extraWidthToReachGridMultiple = 8}
			{@const labelWidthGridCells = Math.ceil(((layerNameLabelWidths?.[String(node.id)] || 0) - extraWidthToReachGridMultiple) / 24)}
			<div
				class="layer"
				class:selected={showSelected($nodeGraph.selected, boxSelection, node.id, nodeIndex)}
				class:previewed={node.previewed}
				class:disabled={node.disabled}
				style:--offset-left={(node.position?.x || 0) + ($nodeGraph.selected.includes(node.id) ? draggingNodes?.roundX || 0 : 0)}
				style:--offset-top={(node.position?.y || 0) + ($nodeGraph.selected.includes(node.id) ? draggingNodes?.roundY || 0 : 0)}
				style:--clip-path-id={`url(#${clipPathId})`}
				style:--data-color={`var(--color-data-${node.primaryOutput?.dataType || "general"})`}
				style:--data-color-dim={`var(--color-data-${node.primaryOutput?.dataType || "general"}-dim)`}
				style:--label-width={labelWidthGridCells}
				data-node={node.id}
				bind:this={nodeElements[nodeIndex]}
			>
				{#if node.errors}
					<span class="node-error faded" transition:fade={FADE_TRANSITION} data-node-error>{node.errors}</span>
					<span class="node-error hover" transition:fade={FADE_TRANSITION} data-node-error>{node.errors}</span>
				{/if}
				<div class="node-chain" />
				<!-- Layer input port (from left) -->
				<div class="input ports">
					<svg
						xmlns="http://www.w3.org/2000/svg"
						viewBox="0 0 8 8"
						class="port"
						data-port="input"
						data-datatype={node.primaryInput?.dataType}
						style:--data-color={`var(--color-data-${node.primaryInput?.dataType})`}
						style:--data-color-dim={`var(--color-data-${node.primaryInput?.dataType}-dim)`}
						bind:this={inputs[nodeIndex][0]}
					>
						{#if node.primaryInput}
							<title>{`${dataTypeTooltip(node.primaryInput)}\nConnected to ${node.primaryInput?.connected || "nothing"}`}</title>
						{/if}
						{#if node.primaryInput?.connected}
							<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color)" />
						{:else}
							<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color-dim)" />
						{/if}
					</svg>
				</div>
				<div class="thumbnail">
					{#if $nodeGraph.thumbnails.has(node.id)}
						{@html $nodeGraph.thumbnails.get(node.id)}
					{/if}
					<!-- Layer stacking top output -->
					{#if node.primaryOutput}
						<svg
							xmlns="http://www.w3.org/2000/svg"
							viewBox="0 0 8 12"
							class="port top"
							data-port="output"
							data-datatype={node.primaryOutput.dataType}
							style:--data-color={`var(--color-data-${node.primaryOutput.dataType})`}
							style:--data-color-dim={`var(--color-data-${node.primaryOutput.dataType}-dim)`}
							bind:this={outputs[nodeIndex][0]}
						>
							<title>{`${dataTypeTooltip(node.primaryOutput)}\nConnected to ${node.primaryOutput.connected || "nothing"}`}</title>
							{#if node.primaryOutput.connected}
								<path d="M0,6.953l2.521,-1.694a2.649,2.649,0,0,1,2.959,0l2.52,1.694v5.047h-8z" fill="var(--data-color)" />
								{#if $nodeGraph.nodes.find((n) => n.id === node.primaryOutput?.connected)?.isLayer}
									<path d="M0,-3.5h8v8l-2.521,-1.681a2.666,2.666,0,0,0,-2.959,0l-2.52,1.681z" fill="var(--data-color-dim)" />
								{/if}
							{:else}
								<path d="M0,6.953l2.521,-1.694a2.649,2.649,0,0,1,2.959,0l2.52,1.694v5.047h-8z" fill="var(--data-color-dim)" />
							{/if}
						</svg>
					{/if}
					<!-- Layer stacking bottom input -->
					<svg
						xmlns="http://www.w3.org/2000/svg"
						viewBox="0 0 8 12"
						class="port bottom"
						data-port="input"
						data-datatype={stackDataInput.dataType}
						style:--data-color={`var(--color-data-${stackDataInput.dataType})`}
						style:--data-color-dim={`var(--color-data-${stackDataInput.dataType}-dim)`}
						bind:this={inputs[nodeIndex][1]}
					>
						<title>{`${dataTypeTooltip(stackDataInput)}\nConnected to ${stackDataInput.connected || "nothing"}`}</title>
						{#if stackDataInput.connected}
							<path d="M0,0H8V8L5.479,6.319a2.666,2.666,0,0,0-2.959,0L0,8Z" fill="var(--data-color)" />
							{#if $nodeGraph.nodes.find((n) => n.id === stackDataInput.connected)?.isLayer}
								<path d="M0,10.95l2.52,-1.69c0.89,-0.6,2.06,-0.6,2.96,0l2.52,1.69v5.05h-8v-5.05z" fill="var(--data-color-dim)" />
							{/if}
						{:else}
							<path d="M0,0H8V8L5.479,6.319a2.666,2.666,0,0,0-2.959,0L0,8Z" fill="var(--data-color-dim)" />
						{/if}
					</svg>
				</div>
				<div class="details">
					<!-- TODO: Allow the user to edit the name, just like in the Layers panel -->
					<span title={editor.instance.inDevelopmentMode() ? `Node ID: ${node.id}` : undefined} bind:offsetWidth={layerNameLabelWidths[String(node.id)]}>
						{node.alias || "Layer"}
					</span>
				</div>
				<IconButton
					class={"visibility"}
					action={(e) => (toggleLayerVisibility(node.id), e?.stopPropagation())}
					size={24}
					icon={node.disabled ? "EyeHidden" : "EyeVisible"}
					tooltip={node.disabled ? "Disabled" : "Enabled"}
				/>

				<svg class="border-mask" width="0" height="0">
					<defs>
						<clipPath id={clipPathId}>
							<!-- Keep this equation in sync with the equivalent one in the CSS rule for `.layer { width: ... }` below -->
							<path clip-rule="evenodd" d={layerBorderMask(36 + 72 + 8 + 24 * Math.max(3, labelWidthGridCells) + 8 + 12 + extraWidthToReachGridMultiple)} />
						</clipPath>
					</defs>
				</svg>
			</div>
		{/each}
		<!-- Nodes -->
		{#each $nodeGraph.nodes.flatMap((node, nodeIndex) => (node.isLayer ? [] : [{ node, nodeIndex }])) as { node, nodeIndex } (nodeIndex)}
			{@const exposedInputsOutputs = [...node.exposedInputs, ...node.exposedOutputs]}
			{@const clipPathId = String(Math.random()).substring(2)}
			<div
				class="node"
				class:selected={showSelected($nodeGraph.selected, boxSelection, node.id, nodeIndex)}
				class:previewed={node.previewed}
				class:disabled={node.disabled}
				style:--offset-left={(node.position?.x || 0) + ($nodeGraph.selected.includes(node.id) ? draggingNodes?.roundX || 0 : 0)}
				style:--offset-top={(node.position?.y || 0) + ($nodeGraph.selected.includes(node.id) ? draggingNodes?.roundY || 0 : 0)}
				style:--clip-path-id={`url(#${clipPathId})`}
				style:--data-color={`var(--color-data-${node.primaryOutput?.dataType || "general"})`}
				style:--data-color-dim={`var(--color-data-${node.primaryOutput?.dataType || "general"}-dim)`}
				data-node={node.id}
				bind:this={nodeElements[nodeIndex]}
			>
				{#if node.errors}
					<span class="node-error faded" transition:fade={FADE_TRANSITION} data-node-error>{node.errors}</span>
					<span class="node-error hover" transition:fade={FADE_TRANSITION} data-node-error>{node.errors}</span>
				{/if}
				<!-- Primary row -->
				<div class="primary" class:no-parameter-section={exposedInputsOutputs.length === 0}>
					<IconLabel icon={nodeIcon(node.name)} />
					<!-- TODO: Allow the user to edit the name, just like in the Layers panel -->
					<TextLabel tooltip={editor.instance.inDevelopmentMode() ? `Node ID: ${node.id}` : undefined}>{node.alias || node.name}</TextLabel>
				</div>
				<!-- Parameter rows -->
				{#if exposedInputsOutputs.length > 0}
					<div class="parameters">
						{#each exposedInputsOutputs as parameter, index}
							<div class={`parameter expanded ${index < node.exposedInputs.length ? "input" : "output"}`}>
								<TextLabel tooltip={parameter.name}>{parameter.name}</TextLabel>
							</div>
						{/each}
					</div>
				{/if}
				<!-- Input ports -->
				<div class="input ports">
					{#if node.primaryInput?.dataType}
						<svg
							xmlns="http://www.w3.org/2000/svg"
							viewBox="0 0 8 8"
							class="port primary-port"
							data-port="input"
							data-datatype={node.primaryInput?.dataType}
							style:--data-color={`var(--color-data-${node.primaryInput?.dataType})`}
							style:--data-color-dim={`var(--color-data-${node.primaryInput?.dataType}-dim)`}
							bind:this={inputs[nodeIndex][0]}
						>
							<title>{`${dataTypeTooltip(node.primaryInput)}\nConnected to ${node.primaryInput.connected || "nothing"}`}</title>
							{#if node.primaryInput.connected}
								<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color)" />
							{:else}
								<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color-dim)" />
							{/if}
						</svg>
					{/if}
					{#each node.exposedInputs as parameter, index}
						{#if index < node.exposedInputs.length}
							<svg
								xmlns="http://www.w3.org/2000/svg"
								viewBox="0 0 8 8"
								class="port"
								data-port="input"
								data-datatype={parameter.dataType}
								style:--data-color={`var(--color-data-${parameter.dataType})`}
								style:--data-color-dim={`var(--color-data-${parameter.dataType}-dim)`}
								bind:this={inputs[nodeIndex][index + (node.primaryInput ? 1 : 0)]}
							>
								<title>{`${dataTypeTooltip(parameter)}\nConnected to ${parameter.connected || "nothing"}`}</title>
								{#if parameter.connected}
									<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color)" />
								{:else}
									<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color-dim)" />
								{/if}
							</svg>
						{/if}
					{/each}
				</div>
				<!-- Output ports -->
				<div class="output ports">
					{#if node.primaryOutput}
						<svg
							xmlns="http://www.w3.org/2000/svg"
							viewBox="0 0 8 8"
							class="port primary-port"
							data-port="output"
							data-datatype={node.primaryOutput.dataType}
							style:--data-color={`var(--color-data-${node.primaryOutput.dataType})`}
							style:--data-color-dim={`var(--color-data-${node.primaryOutput.dataType}-dim)`}
							bind:this={outputs[nodeIndex][0]}
						>
							<title>{`${dataTypeTooltip(node.primaryOutput)}\nConnected to ${node.primaryOutput.connected || "nothing"}`}</title>
							{#if node.primaryOutput.connected}
								<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color)" />
							{:else}
								<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color-dim)" />
							{/if}
						</svg>
					{/if}
					{#each node.exposedOutputs as parameter, outputIndex}
						<svg
							xmlns="http://www.w3.org/2000/svg"
							viewBox="0 0 8 8"
							class="port"
							data-port="output"
							data-datatype={parameter.dataType}
							style:--data-color={`var(--color-data-${parameter.dataType})`}
							style:--data-color-dim={`var(--color-data-${parameter.dataType}-dim)`}
							bind:this={outputs[nodeIndex][outputIndex + (node.primaryOutput ? 1 : 0)]}
						>
							<title>{`${dataTypeTooltip(parameter)}\nConnected to ${parameter.connected || "nothing"}`}</title>
							{#if parameter.connected}
								<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color)" />
							{:else}
								<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color-dim)" />
							{/if}
						</svg>
					{/each}
				</div>
				<svg class="border-mask" width="0" height="0">
					<defs>
						<clipPath id={clipPathId}>
							<path
								clip-rule="evenodd"
								d={nodeBorderMask(120, node.primaryInput?.dataType !== undefined, node.exposedInputs.length, node.primaryOutput !== undefined, node.exposedOutputs.length)}
							/>
						</clipPath>
					</defs>
				</svg>
			</div>
		{/each}
	</div>
</div>

<!-- Box select widget -->
{#if boxSelection}
	<div
		class="box-selection"
		style:left={`${Math.min(boxSelection.startX, boxSelection.endX)}px`}
		style:top={`${Math.min(boxSelection.startY, boxSelection.endY)}px`}
		style:width={`${Math.abs(boxSelection.startX - boxSelection.endX)}px`}
		style:height={`${Math.abs(boxSelection.startY - boxSelection.endY)}px`}
	></div>
{/if}

<style lang="scss" global>
	.graph {
		position: relative;
		overflow: hidden;
		display: flex;
		flex-direction: row;
		flex-grow: 1;

		> img {
			position: absolute;
			bottom: 0;
		}

		// We're displaying the dotted grid in a pseudo-element because `image-rendering` is an inherited property and we don't want it to apply to child elements
		&::before {
			content: "";
			position: absolute;
			width: 100%;
			height: 100%;
			background-size: var(--grid-spacing) var(--grid-spacing);
			background-position: calc(var(--grid-offset-x) - var(--dot-radius)) calc(var(--grid-offset-y) - var(--dot-radius));
			background-image: radial-gradient(circle at var(--dot-radius) var(--dot-radius), var(--color-3-darkgray) var(--dot-radius), transparent 0);
			image-rendering: pixelated;
			mix-blend-mode: screen;
		}

		.node-list {
			width: max-content;
			position: absolute;
			padding: 5px;
			z-index: 3;
			background-color: var(--color-3-darkgray);

			.text-button {
				width: 100%;
			}

			.list-nodes {
				overflow-y: scroll;
			}

			details {
				margin-right: 4px;
				cursor: pointer;
			}

			summary {
				list-style-type: none;
				display: flex;
				align-items: center;
				gap: 2px;

				span {
					white-space: break-spaces;
				}
			}

			details summary svg {
				transform: rotate(-90deg);
			}

			details[open] summary svg {
				transform: rotate(0deg);
			}

			.text-button + .text-button {
				display: block;
				margin-left: 0;
				margin-top: 4px;
			}
		}

		.wires {
			pointer-events: none;
			position: absolute;
			width: 100%;
			height: 100%;

			svg {
				width: 100%;
				height: 100%;
				overflow: visible;

				path {
					fill: none;
					stroke: var(--data-color-dim);
					stroke-width: var(--data-line-width);
				}
			}
		}

		.layers-and-nodes {
			position: absolute;
			width: 100%;
			height: 100%;
		}

		.layer,
		.node {
			position: absolute;
			display: flex;
			left: calc(var(--offset-left) * 24px);
			top: calc(var(--offset-top) * 24px);
			// TODO: Reenable the `transition` property below after dealing with all edge cases where the wires need to be updated until the transition is complete
			// transition: top 0.1s cubic-bezier(0, 0, 0.2, 1), left 0.1s cubic-bezier(0, 0, 0.2, 1); // Update `DRAG_SMOOTHING_TIME` in the JS above.
			// TODO: Reenable the `backdrop-filter` property once a solution can be found for the black whole-page flickering problems it causes in Chrome.
			// TODO: Additionally, find a solution for this having no effect in Firefox due to a browser bug caused when the two
			// ancestor elements, `.graph` and `.panel`, each have the simultaneous pairing of `overflow: hidden` and `border-radius`.
			// See: https://stackoverflow.com/questions/75137879/bug-with-backdrop-filter-in-firefox
			// backdrop-filter: blur(4px);
			background: rgba(0, 0, 0, 0.33);

			.node-error {
				position: absolute;
				width: max-content;
				white-space: pre-wrap;
				max-width: 600px;
				line-height: 18px;
				color: var(--color-2-mildblack);
				background: var(--color-error-red);
				padding: 8px;
				border-radius: 4px;
				bottom: calc(100% + 12px);
				z-index: -1;
				transition: opacity 0.2s ease-in-out;
				opacity: 0.5;

				// Tail
				&::after {
					content: "";
					position: absolute;
					left: 6px;
					bottom: -8px;
					width: 0;
					height: 0;
					border-style: solid;
					border-width: 8px 6px 0 6px;
					border-color: var(--color-error-red) transparent transparent transparent;
				}

				&.hover {
					opacity: 0;
					z-index: 1;
					pointer-events: none;
				}

				&.faded:hover + .hover {
					opacity: 1;
				}

				&.faded:hover {
					z-index: 2;
					opacity: 1;
					-webkit-user-select: text;
					user-select: text;
					transition:
						opacity 0.2s ease-in-out,
						z-index 0s 0.2s;

					&::selection {
						background-color: var(--color-e-nearwhite);

						// Target only Safari
						@supports (background: -webkit-named-image(i)) {
							& {
								// Setting an alpha value opts out of Safari's "fancy" (but not visible on dark backgrounds) selection highlight rendering
								// https://stackoverflow.com/a/71753552/775283
								background-color: rgba(var(--color-e-nearwhite-rgb), calc(254 / 255));
							}
						}
					}
				}
			}

			&::after {
				content: "";
				position: absolute;
				box-sizing: border-box;
				top: 0;
				left: 0;
				width: 100%;
				height: 100%;
				pointer-events: none;
				clip-path: var(--clip-path-id);
			}

			.border-mask {
				position: absolute;
				top: 0;
			}

			&.disabled {
				background: var(--color-3-darkgray);
				color: var(--color-a-softgray);

				.icon-label {
					fill: var(--color-a-softgray);
				}
			}

			&.previewed::after {
				border: 1px dashed var(--data-color);
			}

			.ports {
				position: absolute;

				&.input {
					left: -3px;
				}

				&.output {
					right: -5px;
				}
			}

			.port {
				// Double the intended value because of margin collapsing, but for the first and last we divide it by two as intended
				margin: calc(24px - 8px) 0;
				width: 8px;
				height: 8px;
			}

			.text-label {
				overflow: hidden;
				text-overflow: ellipsis;
			}
		}

		.layer {
			border-radius: 8px;
			--half-visibility-button: 12px;
			--extra-width-to-reach-grid-multiple: 8px;
			// Keep this equation in sync with the equivalent one in the Svelte template `<clipPath><path d="layerBorderMask(...)" /></clipPath>` above
			width: calc(36px + 72px + 8px + 24px * Max(3, var(--label-width)) + 8px + var(--half-visibility-button) + var(--extra-width-to-reach-grid-multiple));

			&::after {
				border: 1px solid var(--color-5-dullgray);
				border-radius: 8px;
			}

			&.selected {
				// This is the result of blending `rgba(255, 255, 255, 0.1)` over `rgba(0, 0, 0, 0.33)`
				background: rgba(66, 66, 66, 0.4);
			}

			.node-chain {
				width: 36px;
			}

			.thumbnail {
				background: var(--color-2-mildblack);
				border: 1px solid var(--data-color-dim);
				border-radius: 2px;
				position: relative;
				box-sizing: border-box;
				width: 72px;
				height: 48px;

				&::before {
					content: "";
					background: var(--color-transparent-checkered-background);
					background-size: var(--color-transparent-checkered-background-size);
					background-position: var(--color-transparent-checkered-background-position);
				}

				&::before,
				svg:not(.port) {
					pointer-events: none;
					position: absolute;
					margin: auto;
					top: 1px;
					left: 1px;
					width: calc(100% - 2px);
					height: calc(100% - 2px);
				}

				.port {
					position: absolute;
					margin: 0 auto;
					left: 0;
					right: 0;
					height: 12px;

					&.top {
						top: -13px;
					}

					&.bottom {
						bottom: -13px;
					}
				}
			}

			.details {
				margin: 0 8px;

				span {
					white-space: nowrap;
					line-height: 48px;
				}
			}

			.visibility {
				position: absolute;
				right: calc(-1 * var(--half-visibility-button));
			}

			.visibility,
			.input.ports,
			.input.ports .port {
				position: absolute;
				margin: auto 0;
				top: 0;
				bottom: 0;
			}

			.input.ports .port {
				left: 24px;
			}
		}

		.node {
			flex-direction: column;
			border-radius: 2px;
			width: 120px;
			top: calc((var(--offset-top) + 0.5) * 24px);

			&::after {
				border: 1px solid var(--data-color-dim);
				border-radius: 2px;
			}

			&.selected {
				.primary {
					background: rgba(255, 255, 255, 0.15);
				}

				.parameters {
					background: rgba(255, 255, 255, 0.1);
				}
			}

			.port {
				&:first-of-type {
					margin-top: calc((24px - 8px) / 2);

					&:not(.primary-port) {
						margin-top: calc((24px - 8px) / 2 + 24px);
					}
				}

				&:last-of-type {
					margin-bottom: calc((24px - 8px) / 2);
				}
			}

			.primary {
				display: flex;
				align-items: center;
				position: relative;
				width: 100%;
				height: 24px;
				border-radius: 2px 2px 0 0;
				background: rgba(255, 255, 255, 0.05);

				&.no-parameter-section {
					border-radius: 2px;
				}

				.icon-label {
					display: none; // Remove after we have unique icons for the nodes
					margin: 0 8px;
				}

				.text-label {
					// margin-right: 4px; // Restore after reenabling icon-label
					margin: 0 8px;
				}
			}

			.parameters {
				display: flex;
				flex-direction: column;
				width: 100%;
				position: relative;

				.parameter {
					position: relative;
					display: flex;
					align-items: center;
					margin: 0 8px;
					width: calc(100% - 8px - 8px);
					height: 24px;

					&:last-of-type {
						border-radius: 0 0 2px 2px;
					}

					.text-label {
						width: 100%;
					}

					&.output {
						flex-direction: row-reverse;
						text-align: right;

						svg {
							width: 30px;
							height: 20px;
						}
					}
				}

				&::before {
					left: 0;
				}

				&::after {
					right: 0;
				}
			}
		}
	}

	.box-selection {
		position: absolute;
		z-index: 2;
		background-color: rgba(77, 168, 221, 0.2);
		border: 1px solid rgba(77, 168, 221);
		pointer-events: none;
	}
</style>

<script lang="ts">
	import { getContext, onMount, tick } from "svelte";

	import type { IconName } from "@graphite/utility-functions/icons";

	import {
		UpdateNodeGraphSelection,
		type FrontendNodeLink,
		type FrontendNodeType,
		type FrontendNode,
		FrontendGraphDataType,
		NodeGraphInput,
		NodeGraphOutput,
	} from "@graphite/wasm-communication/messages";

	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import TextButton from "@graphite/components/widgets/buttons/TextButton.svelte";
	import TextInput from "@graphite/components/widgets/inputs/TextInput.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";
	import WidgetLayout from "@graphite/components/widgets/WidgetLayout.svelte";
	import type { Editor } from "@graphite/wasm-communication/editor";
	import type { NodeGraphState } from "@graphite/state-providers/node-graph";

	const WHEEL_RATE = (1 / 600) * 3;
	const GRID_COLLAPSE_SPACING = 10;
	const GRID_SIZE = 24;
	const ADD_NODE_MENU_WIDTH = 180;
	const ADD_NODE_MENU_HEIGHT = 200;

	const editor = getContext<Editor>("editor");
	const nodeGraph = getContext<NodeGraphState>("nodeGraph");

	let graph: LayoutRow | undefined;
	let nodesContainer: HTMLDivElement | undefined;
	let nodeSearchInput: TextInput | undefined;

	let transform = { scale: 1, x: 0, y: 0 };
	let panning = false;
	let selected: bigint[] = [];
	let draggingNodes: { startX: number; startY: number; roundX: number; roundY: number } | undefined = undefined;
	let selectIfNotDragged: undefined | bigint = undefined;
	let linkInProgressFromConnector: HTMLDivElement | undefined = undefined;
	let linkInProgressToConnector: HTMLDivElement | DOMRect | undefined = undefined;
	let disconnecting: { nodeId: bigint; inputIndex: number; linkIndex: number } | undefined = undefined;
	let nodeLinkPaths: [string, string][] = [];
	let searchTerm = "";
	let nodeListLocation: { x: number; y: number } | undefined = undefined;

	$: watchNodes($nodeGraph.nodes);

	$: gridSpacing = calculateGridSpacing(transform.scale);
	$: dotRadius = 1 + Math.floor(transform.scale - 0.5 + 0.001) / 2;
	$: nodeGraphBarLayout = $nodeGraph.nodeGraphBarLayout;
	$: nodeCategories = buildNodeCategories($nodeGraph.nodeTypes, searchTerm);
	$: nodeListX = ((nodeListLocation?.x || 0) * GRID_SIZE + transform.x) * transform.scale;
	$: nodeListY = ((nodeListLocation?.y || 0) * GRID_SIZE + transform.y) * transform.scale;

	let appearAboveMouse = false;
	let appearRightOfMouse = false;

	$: (() => {
		const bounds = graph?.div()?.getBoundingClientRect();
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

	function createLinkPathInProgress(linkInProgressFromConnector?: HTMLDivElement, linkInProgressToConnector?: HTMLDivElement | DOMRect): [string, string] | undefined {
		if (linkInProgressFromConnector && linkInProgressToConnector && nodesContainer) {
			return createWirePath(linkInProgressFromConnector, linkInProgressToConnector, false, false);
		}
		return undefined;
	}

	function createLinkPaths(linkPathInProgress: [string, string] | undefined, nodeLinkPaths: [string, string][]): [string, string][] {
		const optionalTuple = linkPathInProgress ? [linkPathInProgress] : [];
		return [...optionalTuple, ...nodeLinkPaths];
	}

	async function watchNodes(nodes: FrontendNode[]) {
		selected = selected.filter((id) => nodes.find((node) => node.id === id));
		await refreshLinks();
	}

	function resolveLink(link: FrontendNodeLink, containerBounds: HTMLDivElement): { nodePrimaryOutput: HTMLDivElement | undefined; nodePrimaryInput: HTMLDivElement | undefined } {
		const outputIndex = Number(link.linkStartOutputIndex);
		const inputIndex = Number(link.linkEndInputIndex);

		const nodeOutputConnectors = containerBounds.querySelectorAll(`[data-node="${String(link.linkStart)}"] [data-port="output"]`) || undefined;

		const nodeInputConnectors = containerBounds.querySelectorAll(`[data-node="${String(link.linkEnd)}"] [data-port="input"]`) || undefined;

		const nodePrimaryOutput = nodeOutputConnectors?.[outputIndex] as HTMLDivElement | undefined;
		const nodePrimaryInput = nodeInputConnectors?.[inputIndex] as HTMLDivElement | undefined;
		return { nodePrimaryOutput, nodePrimaryInput };
	}

	async function refreshLinks(): Promise<void> {
		await tick();

		if (!nodesContainer) return;
		const theNodesContainer = nodesContainer;

		const links = $nodeGraph.links;
		nodeLinkPaths = links.flatMap((link, index) => {
			const { nodePrimaryInput, nodePrimaryOutput } = resolveLink(link, theNodesContainer);
			if (!nodePrimaryInput || !nodePrimaryOutput) return [];
			if (disconnecting?.linkIndex === index) return [];

			return [createWirePath(nodePrimaryOutput, nodePrimaryInput.getBoundingClientRect(), false, false)];
		});
	}

	function nodeIcon(nodeName: string): IconName {
		const iconMap: Record<string, IconName> = {
			Output: "NodeOutput",
			Imaginate: "NodeImaginate",
			"Hue Shift Image": "NodeColorCorrection",
			"Brighten Image": "NodeColorCorrection",
			"Grayscale Image": "NodeColorCorrection",
		};
		return iconMap[nodeName] || "NodeNodes";
	}

	function buildWirePathLocations(outputBounds: DOMRect, inputBounds: DOMRect, verticalOut: boolean, verticalIn: boolean): { x: number; y: number }[] {
		if (!nodesContainer) return [];

		const containerBounds = nodesContainer.getBoundingClientRect();

		const outX = verticalOut ? outputBounds.x + outputBounds.width / 2 : outputBounds.x + outputBounds.width - 1;
		const outY = verticalOut ? outputBounds.y + 1 : outputBounds.y + outputBounds.height / 2;
		const outConnectorX = (outX - containerBounds.x) / transform.scale;
		const outConnectorY = (outY - containerBounds.y) / transform.scale;

		const inX = verticalIn ? inputBounds.x + inputBounds.width / 2 : inputBounds.x + 1;
		const inY = verticalIn ? inputBounds.y + inputBounds.height - 1 : inputBounds.y + inputBounds.height / 2;
		const inConnectorX = (inX - containerBounds.x) / transform.scale;
		const inConnectorY = (inY - containerBounds.y) / transform.scale;
		const horizontalGap = Math.abs(outConnectorX - inConnectorX);
		const verticalGap = Math.abs(outConnectorY - inConnectorY);

		const curveLength = 200;
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
		return `M${locations[0].x},${locations[0].y} C${locations[1].x},${locations[1].y} ${locations[2].x},${locations[2].y} ${locations[3].x},${locations[3].y}`;
	}

	function createWirePath(outputPort: HTMLDivElement, inputPort: HTMLDivElement | DOMRect, verticalOut: boolean, verticalIn: boolean): [string, string] {
		const inputPortRect = inputPort instanceof HTMLDivElement ? inputPort.getBoundingClientRect() : inputPort;

		const pathString = buildWirePathString(outputPort.getBoundingClientRect(), inputPortRect, verticalOut, verticalIn);
		const dataType = outputPort.getAttribute("data-datatype") || "general";

		return [pathString, dataType];
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

			const bounds = graph?.div()?.getBoundingClientRect();
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

	function keydown(e: KeyboardEvent): void {
		if (e.key.toLowerCase() === "escape") {
			nodeListLocation = undefined;
			document.removeEventListener("keydown", keydown);
		}
	}

	// TODO: Move the event listener from the graph to the window so dragging outside the graph area (or even the whole browser window) works
	function pointerDown(e: PointerEvent) {
		const [lmb, rmb] = [e.button === 0, e.button === 2];

		const port = (e.target as HTMLDivElement).closest("[data-port]") as HTMLDivElement;
		const node = (e.target as HTMLElement).closest("[data-node]") as HTMLElement | undefined;
		const nodeId = node?.getAttribute("data-node") || undefined;
		const nodeList = (e.target as HTMLElement).closest("[data-node-list]") as HTMLElement | undefined;

		// Create the add node popup on right click, then exit
		if (rmb) {
			const graphBounds = graph?.div()?.getBoundingClientRect();
			if (!graphBounds) return;
			nodeListLocation = {
				x: Math.round(((e.clientX - graphBounds.x) / transform.scale - transform.x) / GRID_SIZE),
				y: Math.round(((e.clientY - graphBounds.y) / transform.scale - transform.y) / GRID_SIZE),
			};

			// Find actual relevant child and focus it (setTimeout is required to actually focus the input element)
			setTimeout(() => nodeSearchInput?.focus(), 0);

			document.addEventListener("keydown", keydown);
			return;
		}

		// If the user is clicking on the add nodes list, exit here
		if (lmb && nodeList) return;

		// Since the user is clicking elsewhere in the graph, ensure the add nodes list is closed
		if (lmb) nodeListLocation = undefined;

		// Alt-click sets the clicked node as previewed
		if (lmb && e.altKey && nodeId) {
			editor.instance.togglePreview(BigInt(nodeId));
		}

		// Clicked on a port dot
		if (lmb && port && node) {
			const isOutput = Boolean(port.getAttribute("data-port") === "output");

			if (isOutput) linkInProgressFromConnector = port;
			else {
				const inputNodeInPorts = Array.from(node.querySelectorAll(`[data-port="input"]`));
				const inputNodeConnectionIndexSearch = inputNodeInPorts.indexOf(port);
				const inputIndex = inputNodeConnectionIndexSearch > -1 ? inputNodeConnectionIndexSearch : undefined;
				// Set the link to draw from the input that a previous link was on
				if (inputIndex !== undefined && nodeId) {
					const nodeIdInt = BigInt(nodeId);
					const inputIndexInt = BigInt(inputIndex);
					const links = $nodeGraph.links;
					const linkIndex = links.findIndex((value) => value.linkEnd === nodeIdInt && value.linkEndInputIndex === inputIndexInt);
					const nodeOutputConnectors = nodesContainer?.querySelectorAll(`[data-node="${String(links[linkIndex].linkStart)}"] [data-port="output"]`) || undefined;
					linkInProgressFromConnector = nodeOutputConnectors?.[Number(links[linkIndex].linkStartOutputIndex)] as HTMLDivElement | undefined;
					const nodeInputConnectors = nodesContainer?.querySelectorAll(`[data-node="${String(links[linkIndex].linkEnd)}"] [data-port="input"]`) || undefined;
					linkInProgressToConnector = nodeInputConnectors?.[Number(links[linkIndex].linkEndInputIndex)] as HTMLDivElement | undefined;
					disconnecting = { nodeId: nodeIdInt, inputIndex, linkIndex };
					refreshLinks();
				}
			}

			return;
		}

		// Clicked on a node
		if (lmb && nodeId) {
			let modifiedSelected = false;

			const id = BigInt(nodeId);
			if (e.shiftKey || e.ctrlKey) {
				modifiedSelected = true;

				if (selected.includes(id)) selected.splice(selected.lastIndexOf(id), 1);
				else selected.push(id);
			} else if (!selected.includes(id)) {
				modifiedSelected = true;

				selected = [id];
			} else {
				selectIfNotDragged = id;
			}

			if (selected.includes(id)) {
				draggingNodes = { startX: e.x, startY: e.y, roundX: 0, roundY: 0 };
			}

			if (modifiedSelected) editor.instance.selectNodes(selected.length > 0 ? new BigUint64Array(selected) : null);

			return;
		}

		// Clicked on the graph background
		if (lmb && selected.length !== 0) {
			selected = [];
			editor.instance.selectNodes(null);
		}

		// LMB clicked on the graph background or MMB clicked anywhere
		panning = true;
	}

	function doubleClick(e: MouseEvent) {
		// const node = (e.target as HTMLElement).closest("[data-node]") as HTMLElement | undefined;
		// const nodeId = node?.getAttribute("data-node") || undefined;
		// if (nodeId) {
		// 	const id = BigInt(nodeId);
		// 	editor.instance.doubleClickNode(id);
		// }
	}

	function pointerMove(e: PointerEvent) {
		if (panning) {
			transform.x += e.movementX / transform.scale;
			transform.y += e.movementY / transform.scale;
		} else if (linkInProgressFromConnector) {
			const target = e.target as Element | undefined;
			const dot = (target?.closest(`[data-port="input"]`) || undefined) as HTMLDivElement | undefined;
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
				refreshLinks();
			}
		}
	}

	function pointerUp(e: PointerEvent) {
		panning = false;

		if (disconnecting) {
			editor.instance.disconnectNodes(BigInt(disconnecting.nodeId), disconnecting.inputIndex);
		}
		disconnecting = undefined;

		if (linkInProgressToConnector instanceof HTMLDivElement && linkInProgressFromConnector) {
			const outputNode = linkInProgressFromConnector.closest("[data-node]");
			const inputNode = linkInProgressToConnector.closest("[data-node]");

			const outputConnectedNodeID = outputNode?.getAttribute("data-node") ?? undefined;
			const inputConnectedNodeID = inputNode?.getAttribute("data-node") ?? undefined;

			if (outputNode && inputNode && outputConnectedNodeID && inputConnectedNodeID) {
				const inputNodeInPorts = Array.from(inputNode.querySelectorAll(`[data-port="input"]`));
				const outputNodeInPorts = Array.from(outputNode.querySelectorAll(`[data-port="output"]`));

				const inputNodeConnectionIndexSearch = inputNodeInPorts.indexOf(linkInProgressToConnector);
				const outputNodeConnectionIndexSearch = outputNodeInPorts.indexOf(linkInProgressFromConnector);

				const inputNodeConnectionIndex = inputNodeConnectionIndexSearch > -1 ? inputNodeConnectionIndexSearch : undefined;
				const outputNodeConnectionIndex = outputNodeConnectionIndexSearch > -1 ? outputNodeConnectionIndexSearch : undefined;

				if (inputNodeConnectionIndex !== undefined && outputNodeConnectionIndex !== undefined) {
					editor.instance.connectNodesByLink(BigInt(outputConnectedNodeID), outputNodeConnectionIndex, BigInt(inputConnectedNodeID), inputNodeConnectionIndex);
				}
			}
		} else if (draggingNodes) {
			if (draggingNodes.startX === e.x || draggingNodes.startY === e.y) {
				if (selectIfNotDragged !== undefined && (selected.length !== 1 || selected[0] !== selectIfNotDragged)) {
					selected = [selectIfNotDragged];
					editor.instance.selectNodes(new BigUint64Array(selected));
				}
			}

			if (selected.length > 0 && (draggingNodes.roundX !== 0 || draggingNodes.roundY !== 0)) editor.instance.moveSelectedNodes(draggingNodes.roundX, draggingNodes.roundY);

			// Check if this node should be inserted between two other nodes
			if (selected.length === 1) {
				const selectedNodeId = selected[0];
				const selectedNode = nodesContainer?.querySelector(`[data-node="${String(selectedNodeId)}"]`) || undefined;

				// Check that neither the input or output of the selected node are already connected.
				const notConnected = $nodeGraph.links.findIndex((link) => link.linkStart === selectedNodeId || (link.linkEnd === selectedNodeId && link.linkEndInputIndex === BigInt(0))) === -1;
				const input = selectedNode?.querySelector(`[data-port="input"]`) || undefined;
				const output = selectedNode?.querySelector(`[data-port="output"]`) || undefined;

				// TODO: Make sure inputs are correctly typed
				if (selectedNode && notConnected && input && output && nodesContainer) {
					const theNodesContainer = nodesContainer;

					// Find the link that the node has been dragged on top of
					const link = $nodeGraph.links.find((link): boolean => {
						const { nodePrimaryInput, nodePrimaryOutput } = resolveLink(link, theNodesContainer);
						if (!nodePrimaryInput || !nodePrimaryOutput) return false;

						const wireCurveLocations = buildWirePathLocations(nodePrimaryOutput.getBoundingClientRect(), nodePrimaryInput.getBoundingClientRect(), false, false);

						const selectedNodeBounds = selectedNode.getBoundingClientRect();
						const containerBoundsBounds = theNodesContainer.getBoundingClientRect();

						return editor.instance.rectangleIntersects(
							new Float64Array(wireCurveLocations.map((loc) => loc.x)),
							new Float64Array(wireCurveLocations.map((loc) => loc.y)),
							selectedNodeBounds.top - containerBoundsBounds.y,
							selectedNodeBounds.left - containerBoundsBounds.x,
							selectedNodeBounds.bottom - containerBoundsBounds.y,
							selectedNodeBounds.right - containerBoundsBounds.x
						);
					});

					// If the node has been dragged on top of the link then connect it into the middle.
					if (link) {
						editor.instance.connectNodesByLink(link.linkStart, 0, selectedNodeId, 0);
						editor.instance.connectNodesByLink(selectedNodeId, 0, link.linkEnd, Number(link.linkEndInputIndex));
						editor.instance.shiftNode(selectedNodeId);
					}
				}
			}

			draggingNodes = undefined;
			selectIfNotDragged = undefined;
		}

		linkInProgressFromConnector = undefined;
		linkInProgressToConnector = undefined;
	}

	function createNode(nodeType: string): void {
		if (!nodeListLocation) return;

		editor.instance.createNode(nodeType, nodeListLocation.x, nodeListLocation.y);
		nodeListLocation = undefined;
	}

	function buildBorderMask(nodeWidth: number, primaryInputExists: boolean, parameters: number, outputsIncludingPrimary: number): string {
		const nodeHeight = Math.max(1 + parameters, outputsIncludingPrimary) * 24;

		const boxes: { x: number; y: number; width: number; height: number }[] = [];
		if (primaryInputExists) boxes.push({ x: -8, y: 4, width: 16, height: 16 });
		for (let i = 0; i < parameters; i++) boxes.push({ x: -8, y: 4 + (i + 1) * 24, width: 16, height: 16 });
		for (let i = 0; i < outputsIncludingPrimary; i++) boxes.push({ x: nodeWidth - 8, y: 4 + i * 24, width: 16, height: 16 });

		const rectangles = boxes.map((box) => `M${box.x},${box.y} L${box.x + box.width},${box.y} L${box.x + box.width},${box.y + box.height} L${box.x},${box.y + box.height}z`);
		return `M-2,-2 L${nodeWidth + 2},-2 L${nodeWidth + 2},${nodeHeight + 2} L-2,${nodeHeight + 2}z ${rectangles.join(" ")}`;
	}

	onMount(() => {
		const outputPort1 = document.querySelectorAll(`[data-port="output"]`)[4] as HTMLDivElement | undefined;
		const inputPort1 = document.querySelectorAll(`[data-port="input"]`)[1] as HTMLDivElement | undefined;
		if (outputPort1 && inputPort1) createWirePath(outputPort1, inputPort1.getBoundingClientRect(), true, true);

		const outputPort2 = document.querySelectorAll(`[data-port="output"]`)[6] as HTMLDivElement | undefined;
		const inputPort2 = document.querySelectorAll(`[data-port="input"]`)[3] as HTMLDivElement | undefined;
		if (outputPort2 && inputPort2) createWirePath(outputPort2, inputPort2.getBoundingClientRect(), true, false);

		editor.subscriptions.subscribeJsMessage(UpdateNodeGraphSelection, (updateNodeGraphSelection) => {
			selected = updateNodeGraphSelection.selected;
		});
	});
</script>

<LayoutCol class="node-graph">
	<LayoutRow class="options-bar"><WidgetLayout layout={nodeGraphBarLayout} /></LayoutRow>
	<LayoutRow
		class="graph"
		bind:this={graph}
		on:wheel={scroll}
		on:pointerdown={pointerDown}
		on:pointermove={pointerMove}
		on:pointerup={pointerUp}
		on:dblclick={doubleClick}
		styles={{
			"--grid-spacing": `${gridSpacing}px`,
			"--grid-offset-x": `${transform.x * transform.scale}px`,
			"--grid-offset-y": `${transform.y * transform.scale}px`,
			"--dot-radius": `${dotRadius}px`,
		}}
	>
		<img src="https://files.keavon.com/-/MountainousDroopyBlueshark/flyover.jpg" />
		<div class="fade-artwork" />
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
				<div class="list-nodes" style={`height: ${ADD_NODE_MENU_HEIGHT}px;`} on:wheel|stopPropagation>
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
				{#each linkPaths as [pathString, dataType]}
					<path d={pathString} style:--data-color={`var(--color-data-${dataType})`} style:--data-color-dim={`var(--color-data-${dataType}-dim)`} />
				{/each}
			</svg>
		</div>
		<!-- Nodes -->
		<div class="nodes" style:transform={`scale(${transform.scale}) translate(${transform.x}px, ${transform.y}px)`} style:transform-origin={`0 0`} bind:this={nodesContainer}>
			{#each $nodeGraph.nodes as node (String(node.id))}
				{@const exposedInputsOutputs = [...node.exposedInputs, ...node.outputs.slice(1)]}
				{@const clipPathId = `${Math.random()}`.substring(2)}
				<div
					class="node"
					class:selected={selected.includes(node.id)}
					class:previewed={node.previewed}
					class:disabled={node.disabled}
					class:is-layer={node.thumbnailSvg !== undefined}
					style:--offset-left={(node.position?.x || 0) + (selected.includes(node.id) ? draggingNodes?.roundX || 0 : 0)}
					style:--offset-top={(node.position?.y || 0) + (selected.includes(node.id) ? draggingNodes?.roundY || 0 : 0)}
					style:--clip-path-id={`url(#${clipPathId})`}
					data-node={node.id}
				>
					<!-- Primary row -->
					<div class="primary" class:no-parameter-section={exposedInputsOutputs.length === 0}>
						{#if node.thumbnailSvg}
							{@html node.thumbnailSvg}
						{:else}
							<IconLabel icon={nodeIcon(node.displayName)} />
						{/if}
						<TextLabel>{node.displayName}</TextLabel>
					</div>
					<!-- Parameter rows -->
					{#if exposedInputsOutputs.length > 0}
						<div class="parameters">
							{#each exposedInputsOutputs as parameter, index}
								<div class="parameter expanded">
									<div class="expand-arrow" />
									<TextLabel class={index < node.exposedInputs.length ? "name" : "output"}>{parameter.name}</TextLabel>
								</div>
							{/each}
						</div>
					{/if}
					<!-- Input ports -->
					<div class="input ports">
						{#if node.primaryInput}
							<svg
								xmlns="http://www.w3.org/2000/svg"
								viewBox="0 0 8 8"
								class="port"
								data-port="input"
								data-datatype={node.primaryInput}
								style:--data-color={`var(--color-data-${node.primaryInput})`}
								style:--data-color-dim={`var(--color-data-${node.primaryInput}-dim)`}
							>
								<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" />
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
								>
									<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" />
								</svg>
							{/if}
						{/each}
					</div>
					<!-- Output ports -->
					<div class="output ports">
						{#if node.outputs.length > 0}
							<svg
								xmlns="http://www.w3.org/2000/svg"
								viewBox="0 0 8 8"
								class="port"
								data-port="output"
								data-datatype={node.outputs[0].dataType}
								style:--data-color={`var(--color-data-${node.outputs[0].dataType})`}
								style:--data-color-dim={`var(--color-data-${node.outputs[0].dataType}-dim)`}
							>
								<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" />
							</svg>
						{/if}
						{#each node.outputs.slice(1) as parameter}
							<svg
								xmlns="http://www.w3.org/2000/svg"
								viewBox="0 0 8 8"
								class="port"
								data-port="output"
								data-datatype={parameter.dataType}
								style:--data-color={`var(--color-data-${parameter.dataType})`}
								style:--data-color-dim={`var(--color-data-${parameter.dataType}-dim)`}
							>
								<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" />
							</svg>
						{/each}
					</div>
					<svg class="border-mask" width="0" height="0">
						<defs>
							<clipPath id={clipPathId}>
								<path clip-rule="evenodd" d={buildBorderMask(120, node.primaryInput !== undefined, node.exposedInputs.length, node.outputs.length)} />
							</clipPath>
						</defs>
					</svg>
				</div>
			{/each}
		</div>
	</LayoutRow>
</LayoutCol>

<style lang="scss" global>
	.node-graph {
		height: 100%;
		position: relative;

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

		.options-bar {
			height: 32px;
			margin: 0 4px;
			flex: 0 0 auto;
			align-items: center;

			.widget-layout {
				flex-direction: row;
				flex-grow: 1;
				justify-content: space-between;
			}
		}

		.fade-artwork {
			background: var(--color-2-mildblack);
			opacity: 0.8;
			width: 100%;
			height: 100%;
		}

		.graph {
			position: relative;
			background: var(--color-2-mildblack);
			width: calc(100% - 8px);
			margin-left: 4px;
			margin-bottom: 4px;
			border-radius: 2px;
			overflow: hidden;

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
		}

		.nodes,
		.wires {
			position: absolute;
			width: 100%;
			height: 100%;

			&.wires {
				width: 100%;
				height: 100%;
				pointer-events: none;

				svg {
					width: 100%;
					height: 100%;
					overflow: visible;

					path {
						fill: none;
						stroke: var(--data-color-dim);
						stroke-width: 2px;
					}
				}
			}

			&.nodes {
				.node {
					position: absolute;
					display: flex;
					flex-direction: column;
					min-width: 120px;
					border-radius: 2px;
					left: calc((var(--offset-left) + 0.5) * 24px);
					top: calc((var(--offset-top) - 0.5) * 24px);
					backdrop-filter: blur(8px) brightness(100% - 33%);

					&::after {
						content: "";
						position: absolute;
						box-sizing: border-box;
						top: 0;
						left: 0;
						width: 100%;
						height: 100%;
						border: 1px solid var(--color-data-vector-dim);
						border-radius: 2px;
						pointer-events: none;
						clip-path: var(--clip-path-id);
					}

					.primary {
						display: flex;
						align-items: center;
						position: relative;
						width: 100%;
						height: 24px;
						border-radius: 2px 2px 0 0;
						font-style: italic;
						background: rgba(255, 255, 255, 0.05);

						&.no-parameter-section {
							border-radius: 2px;
						}

						.icon-label {
							margin: 0 8px;
						}

						.text-label {
							margin-right: 4px;
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
							width: 100%;
							height: 24px;

							&:last-of-type {
								border-radius: 0 0 2px 2px;
							}

							.expand-arrow {
								margin-left: 4px;
							}

							.text-label {
								width: 100%;

								&.output {
									text-align: right;
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

					.border-mask {
						position: absolute;
						top: 0;
					}

					&.selected {
						.primary {
							background: rgba(255, 255, 255, 0.15);
						}

						.parameters {
							background: rgba(255, 255, 255, 0.1);
						}
					}

					&.disabled {
						background: var(--color-3-darkgray);
						color: var(--color-a-softgray);

						.icon-label {
							fill: var(--color-a-softgray);
						}

						.expand-arrow::after {
							background: var(--icon-expand-collapse-arrow-disabled);
						}
					}

					&.previewed::after {
						border: 1px dashed var(--color-data-vector);
					}

					.ports {
						position: absolute;

						&.input {
							left: -3px;
						}

						&.output {
							right: -5px;
						}

						.port {
							fill: var(--data-color);
							// Double the intended value because of margin collapsing, but for the first and last we divide it by two as intended
							margin: calc(24px - 8px) 0;
							width: 8px;
							height: 8px;

							&:first-of-type {
								margin-top: calc((24px - 8px) / 2);
							}

							&:last-of-type {
								margin-bottom: calc((24px - 8px) / 2);
							}
						}
					}

					.expand-arrow {
						width: 16px;
						height: 16px;
						margin: 0;
						padding: 0;
						position: relative;
						flex: 0 0 auto;
						display: flex;
						align-items: center;
						justify-content: center;

						&::after {
							content: "";
							position: absolute;
							width: 8px;
							height: 8px;
							background: var(--icon-expand-collapse-arrow);
						}

						&:hover::after {
							background: var(--icon-expand-collapse-arrow-hover);
						}
					}

					.expanded .expand-arrow::after {
						transform: rotate(90deg);
					}
				}

				.node.is-layer {
					.primary svg {
						width: 24px;
						height: 24px;
					}
				}
			}
		}
	}
</style>

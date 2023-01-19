<script lang="ts">
	import { getContext, onMount, tick } from "svelte";

	import type { IconName } from "@/utility-functions/icons";

	import { UpdateNodeGraphSelection, type FrontendNodeLink, type FrontendNodeType, type FrontendNode } from "@/wasm-communication/messages";

	import LayoutCol from "@/components/layout/LayoutCol.svelte";
	import LayoutRow from "@/components/layout/LayoutRow.svelte";
	import TextButton from "@/components/widgets/buttons/TextButton.svelte";
	import TextInput from "@/components/widgets/inputs/TextInput.svelte";
	import IconLabel from "@/components/widgets/labels/IconLabel.svelte";
	import TextLabel from "@/components/widgets/labels/TextLabel.svelte";
	import WidgetLayout from "@/components/widgets/WidgetLayout.svelte";
	import type { Editor } from "@/wasm-communication/editor";
	import type { NodeGraphState } from "@/state-providers/node-graph";

	const WHEEL_RATE = (1 / 600) * 3;
	const GRID_COLLAPSE_SPACING = 10;
	const GRID_SIZE = 24;

	const editor = getContext<Editor>("editor");
	const nodeGraph = getContext<NodeGraphState>("nodeGraph");

	let graph: LayoutRow;
	let nodesContainer: HTMLDivElement;
	let nodeSearchInput: TextInput;
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

	$: gridSpacing = calculateGridSpacing(transform.scale);
	$: dotRadius = 1 + Math.floor(transform.scale - 0.5 + 0.001) / 2;
	$: nodeGraphBarLayout = $nodeGraph.nodeGraphBarLayout;
	$: nodeCategories = buildNodeCategories($nodeGraph.nodeTypes, searchTerm);
	$: nodeListX = ((nodeListLocation?.x || 0) * GRID_SIZE + transform.x) * transform.scale;
	$: nodeListY = ((nodeListLocation?.y || 0) * GRID_SIZE + transform.y) * transform.scale;
	$: linkPathInProgress = createLinkPathInProgress(linkInProgressFromConnector, linkInProgressToConnector);
	$: linkPaths = createLinkPaths(linkPathInProgress, nodeLinkPaths);

	$: watchNodes($nodeGraph.nodes);

	function calculateGridSpacing(scale: number): number {
		const dense = scale * GRID_SIZE;
		let sparse = dense;

		while (sparse > 0 && sparse < GRID_COLLAPSE_SPACING) {
			sparse *= 2;
		}

		return sparse;
	}

	function buildNodeCategories(nodeTypes: FrontendNodeType[], searchTerm: string) {
		const categories = new Map();
		nodeTypes.forEach((node) => {
			if (searchTerm.length > 0 && !node.name.toLowerCase().includes(searchTerm.toLowerCase()) && !node.category.toLowerCase().includes(searchTerm.toLowerCase())) {
				return;
			}

			const category = categories.get(node.category);
			if (category) category.push(node);
			else categories.set(node.category, [node]);
		});

		return Array.from(categories);
	}

	function createLinkPathInProgress(linkInProgressFromConnector?: HTMLDivElement, linkInProgressToConnector?: HTMLDivElement | DOMRect): [string, string] | undefined {
		if (linkInProgressFromConnector && linkInProgressToConnector) {
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

		const links = $nodeGraph.links;
		nodeLinkPaths = links.flatMap((link, index) => {
			const { nodePrimaryInput, nodePrimaryOutput } = resolveLink(link, nodesContainer);
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
		const scrollX = e.deltaX;
		const scrollY = e.deltaY;

		// Zoom
		if (e.ctrlKey) {
			let zoomFactor = 1 + Math.abs(scrollY) * WHEEL_RATE;
			if (scrollY > 0) zoomFactor = 1 / zoomFactor;

			const { x, y, width, height } = graph.div().getBoundingClientRect();

			transform.scale *= zoomFactor;

			const newViewportX = width / zoomFactor;
			const newViewportY = height / zoomFactor;

			const deltaSizeX = width - newViewportX;
			const deltaSizeY = height - newViewportY;

			const deltaX = deltaSizeX * ((e.x - x) / width);
			const deltaY = deltaSizeY * ((e.y - y) / height);

			transform.x -= (deltaX / transform.scale) * zoomFactor;
			transform.y -= (deltaY / transform.scale) * zoomFactor;

			// Prevent actually zooming into the page when pinch-zooming on laptop trackpads
			e.preventDefault();
		}
		// Pan
		else if (!e.shiftKey) {
			transform.x -= scrollX / transform.scale;
			transform.y -= scrollY / transform.scale;
		} else {
			transform.x -= scrollY / transform.scale;
		}
	}

	function keydown(e: KeyboardEvent): void {
		if (e.key.toLowerCase() === "escape") {
			nodeListLocation = undefined;
			document.removeEventListener("keydown", keydown);
		}
	}

	// TODO: Move the event listener from the graph to the window so dragging outside the graph area (or even the browser window) works
	function pointerDown(e: PointerEvent) {
		// Exit the add node popup by clicking elsewhere in the graph
		if (nodeListLocation && !(e.target as HTMLElement).closest("[data-node-list]")) nodeListLocation = undefined;

		// Handle the add node popup on right click
		if (e.button === 2) {
			const graphBounds = graph.div().getBoundingClientRect();
			nodeListLocation = {
				x: Math.round(((e.clientX - graphBounds.x) / transform.scale - transform.x) / GRID_SIZE),
				y: Math.round(((e.clientY - graphBounds.y) / transform.scale - transform.y) / GRID_SIZE),
			};

			// Find actual relevant child and focus it
			// TODO: Svelte: check if this works and if `setTimeout` can be removed
			setTimeout(() => nodeSearchInput.focus(), 0);

			document.addEventListener("keydown", keydown);
			return;
		}

		const port = (e.target as HTMLDivElement).closest("[data-port]") as HTMLDivElement;
		const node = (e.target as HTMLElement).closest("[data-node]") as HTMLElement | undefined;
		const nodeId = node?.getAttribute("data-node") || undefined;
		const nodeList = (e.target as HTMLElement).closest("[data-node-list]") as HTMLElement | undefined;

		// If the user is clicking on the add nodes list, exit here
		if (nodeList) return;

		if (e.altKey && nodeId) {
			editor.instance.togglePreview(BigInt(nodeId));
		}

		// Clicked on a port dot
		if (port && node) {
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
					const nodeOutputConnectors = nodesContainer.querySelectorAll(`[data-node="${String(links[linkIndex].linkStart)}"] [data-port="output"]`) || undefined;
					linkInProgressFromConnector = nodeOutputConnectors?.[Number(links[linkIndex].linkEndInputIndex)] as HTMLDivElement | undefined;
					const nodeInputConnectors = nodesContainer.querySelectorAll(`[data-node="${String(links[linkIndex].linkEnd)}"] [data-port="input"]`) || undefined;
					linkInProgressToConnector = nodeInputConnectors?.[Number(links[linkIndex].linkEndInputIndex)] as HTMLDivElement | undefined;
					disconnecting = { nodeId: nodeIdInt, inputIndex, linkIndex };
					refreshLinks();
				}
			}

			return;
		}

		// Clicked on a node
		if (nodeId) {
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

			if (modifiedSelected) editor.instance.selectNodes(new BigUint64Array(selected));

			return;
		}

		// Clicked on the graph background
		panning = true;
		if (selected.length !== 0) {
			selected = [];
			editor.instance.selectNodes(new BigUint64Array(selected));
		}
	}

	function doubleClick(e: MouseEvent) {
		const node = (e.target as HTMLElement).closest("[data-node]") as HTMLElement | undefined;
		const nodeId = node?.getAttribute("data-node") || undefined;
		if (nodeId) {
			const id = BigInt(nodeId);
			editor.instance.doubleClickNode(id);
		}
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
				const selectedNode = nodesContainer.querySelector(`[data-node="${String(selectedNodeId)}"]`);

				// Check that neither the input or output of the selected node are already connected.
				const notConnected = $nodeGraph.links.findIndex((link) => link.linkStart === selectedNodeId || (link.linkEnd === selectedNodeId && link.linkEndInputIndex === BigInt(0))) === -1;
				const input = selectedNode?.querySelector(`[data-port="input"]`);
				const output = selectedNode?.querySelector(`[data-port="output"]`);

				// TODO: Make sure inputs are correctly typed
				if (selectedNode && notConnected && input && output) {
					// Find the link that the node has been dragged on top of
					const link = $nodeGraph.links.find((link): boolean => {
						const { nodePrimaryInput, nodePrimaryOutput } = resolveLink(link, nodesContainer);
						if (!nodePrimaryInput || !nodePrimaryOutput) return false;

						const wireCurveLocations = buildWirePathLocations(nodePrimaryOutput.getBoundingClientRect(), nodePrimaryInput.getBoundingClientRect(), false, false);

						const selectedNodeBounds = selectedNode.getBoundingClientRect();
						const containerBoundsBounds = nodesContainer.getBoundingClientRect();

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
		{#if nodeListLocation}
			<LayoutCol class="node-list" data-node-list styles={{ "margin-left": `${nodeListX}px`, "margin-top": `${nodeListY}px` }}>
				<TextInput placeholder="Search Nodes..." value={searchTerm} on:value={({ detail }) => (searchTerm = detail)} bind:this={nodeSearchInput} />
				{#each nodeCategories as nodeCategory (nodeCategory[0])}
					<LayoutCol>
						<TextLabel>{nodeCategory[0]}</TextLabel>
						{#each nodeCategory[1] as nodeType (String(nodeType))}
							<TextButton label={nodeType.name} action={() => createNode(nodeType.name)} />
						{/each}
					</LayoutCol>
				{:else}
					<TextLabel>No search results</TextLabel>
				{/each}
			</LayoutCol>
		{/if}
		<div class="nodes" style:transform={`scale(${transform.scale}) translate(${transform.x}px, ${transform.y}px)`} style:transform-origin={`0 0`} bind:this={nodesContainer}>
			{#each $nodeGraph.nodes as node (String(node.id))}
				{@const exposedInputsOutputs = [...node.exposedInputs, ...node.outputs.slice(1)]}
				<div
					class="node"
					class:selected={selected.includes(node.id)}
					class:previewed={node.previewed}
					class:disabled={node.disabled}
					style:--offset-left={(node.position?.x || 0) + (selected.includes(node.id) ? draggingNodes?.roundX || 0 : 0)}
					style:--offset-top={(node.position?.y || 0) + (selected.includes(node.id) ? draggingNodes?.roundY || 0 : 0)}
					data-node={node.id}
				>
					<div class="primary">
						<div class="ports">
							{#if node.primaryInput}
								<div
									class="input port"
									data-port="input"
									data-datatype={node.primaryInput}
									style:--data-color={`var(--color-data-${node.primaryInput})`}
									style:--data-color-dim={`var(--color-data-${node.primaryInput}-dim)`}
								>
									<div />
								</div>
							{/if}
							{#if node.outputs.length > 0}
								<div
									class="output port"
									data-port="output"
									data-datatype={node.outputs[0].dataType}
									style:--data-color={`var(--color-data-${node.outputs[0].dataType})`}
									style:--data-color-dim={`var(--color-data-${node.outputs[0].dataType}-dim)`}
								>
									<div />
								</div>
							{/if}
						</div>
						<IconLabel icon={nodeIcon(node.displayName)} />
						<TextLabel>{node.displayName}</TextLabel>
					</div>
					{#if exposedInputsOutputs.length > 0}
						<div class="parameters">
							{#each exposedInputsOutputs as parameter, index (index)}
								<div class="parameter">
									<div class="ports">
										{#if index < node.exposedInputs.length}
											<div
												class="input port"
												data-port="input"
												data-datatype={parameter.dataType}
												style:--data-color={`var(--color-data-${parameter.dataType})`}
												style:--data-color-dim={`var(--color-data-${parameter.dataType}-dim)`}
											>
												<div />
											</div>
										{:else}
											<div
												class="output port"
												data-port="output"
												data-datatype={parameter.dataType}
												style:--data-color={`var(--color-data-${parameter.dataType})`}
												style:--data-color-dim={`var(--color-data-${parameter.dataType}-dim)`}
											>
												<div />
											</div>
										{/if}
									</div>
									<TextLabel class={index < node.exposedInputs.length ? "name" : "output"}>{parameter.name}</TextLabel>
								</div>
							{/each}
						</div>
					{/if}
				</div>
			{/each}
		</div>
		<div class="wires" style:transform={`scale(${transform.scale}) translate(${transform.x}px, ${transform.y}px)`} style:transform-origin={`0 0`}>
			<svg>
				{#each linkPaths as [pathString, dataType], index (index)}
					<path d={pathString} style:--data-color={`var(--color-data-${dataType})`} style:--data-color-dim={`var(--color-data-${dataType}-dim)`} />
				{/each}
			</svg>
		</div>
	</LayoutRow>
</LayoutCol>

<style lang="scss" global>
	.node-graph {
		height: 100%;
		position: relative;

		.node-list {
			width: max-content;
			position: fixed;
			padding: 5px;
			z-index: 3;
			background-color: var(--color-3-darkgray);

			.text-button + .text-button {
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

		.graph {
			position: relative;
			background: var(--color-2-mildblack);
			width: calc(100% - 8px);
			margin-left: 4px;
			margin-bottom: 4px;
			border-radius: 2px;
			overflow: hidden;

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
						// stroke: var(--color-data-raster-dim);
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
					border-radius: 4px;
					background: var(--color-4-dimgray);
					left: calc((var(--offset-left) + 0.5) * 24px);
					top: calc((var(--offset-top) - 0.5) * 24px);

					&.selected {
						border: 1px solid var(--color-e-nearwhite);
						margin: -1px;
					}

					&.disabled {
						background: var(--color-3-darkgray);
						color: var(--color-a-softgray);

						.icon-label {
							fill: var(--color-a-softgray);
						}
					}

					&.previewed {
						outline: 3px solid var(--color-data-vector);
					}

					.primary {
						display: flex;
						align-items: center;
						position: relative;
						gap: 4px;
						width: 100%;
						height: 24px;
						background: var(--color-5-dullgray);
						border-radius: 4px;

						.icon-label {
							margin-left: 4px;
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
							height: 24px;
							width: calc(100% - 24px * 2);
							margin-left: 24px;
							margin-right: 24px;

							.text-label {
								width: 100%;

								&.output {
									text-align: right;
								}
							}
						}

						// Squares to cover up the rounded corners of the primary area and make them have a straight edge
						&::before,
						&::after {
							content: "";
							position: absolute;
							background: var(--color-5-dullgray);
							width: 4px;
							height: 4px;
							top: -4px;
						}

						&::before {
							left: 0;
						}

						&::after {
							right: 0;
						}
					}

					.ports {
						position: absolute;
						width: 100%;
						height: 100%;

						.port {
							position: absolute;
							margin: auto 0;
							top: 0;
							bottom: 0;
							width: 12px;
							height: 12px;
							border-radius: 50%;
							background: var(--data-color-dim);
							// background: var(--color-data-raster-dim);

							div {
								background: var(--data-color);
								// background: var(--color-data-raster);
								width: 8px;
								height: 8px;
								border-radius: 50%;
								position: absolute;
								top: 0;
								bottom: 0;
								left: 0;
								right: 0;
								margin: auto;
							}

							&.input {
								left: calc(-12px - 6px);
							}

							&.output {
								right: calc(-12px - 6px);
							}
						}
					}
				}
			}
		}
	}
</style>

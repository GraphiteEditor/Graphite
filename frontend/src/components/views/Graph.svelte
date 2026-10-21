<script lang="ts">
	import { getContext, onDestroy, onMount, tick } from "svelte";
	import { cubicInOut } from "svelte/easing";
	import { fade } from "svelte/transition";
	import NodeCatalog from "/src/components/floating-menus/NodeCatalog.svelte";
	import FloatingMenu from "/src/components/layout/FloatingMenu.svelte";
	import LayoutCol from "/src/components/layout/LayoutCol.svelte";
	import IconButton from "/src/components/widgets/buttons/IconButton.svelte";
	import TextButton from "/src/components/widgets/buttons/TextButton.svelte";
	import TextAreaInput from "/src/components/widgets/inputs/TextAreaInput.svelte";
	import IconLabel from "/src/components/widgets/labels/IconLabel.svelte";
	import TextLabel from "/src/components/widgets/labels/TextLabel.svelte";
	import type { DocumentStore } from "/src/stores/document";
	import type { NodeGraphStore } from "/src/stores/node-graph";
	import { closeContextMenu } from "/src/stores/node-graph";
	import type { SubscriptionsRouter } from "/src/subscriptions-router";
	import { operatingSystem } from "/src/utility-functions/platform";
	import type { EditorWrapper, FrontendGraphInput, FrontendGraphOutput, FrontendNode } from "/wrapper/pkg/graphite_wasm_wrapper";

	const GRID_COLLAPSE_SPACING = 10;
	const GRID_SIZE = 24;
	const FADE_TRANSITION = { duration: 200, easing: cubicInOut };

	const editor = getContext<EditorWrapper>("editor");
	const nodeGraph = getContext<NodeGraphStore>("nodeGraph");
	const nodeGraphTransform = nodeGraph.transformStore;
	const nodeGraphImportsExports = nodeGraph.importsExportsStore;
	const visibleNodes = nodeGraph.visibleNodesStore;
	const nodeGraphWires = nodeGraph.wiresStore;
	const wireInsertionPreview = nodeGraph.wireInsertionPreviewStore;
	const nodeGlideOffsets = nodeGraph.glideOffsetsStore;
	const documentState = getContext<DocumentStore>("document");
	const subscriptions = getContext<SubscriptionsRouter>("subscriptions");

	$: gridSpacing = calculateGridSpacing($nodeGraphTransform.scale);
	$: gridDotRadius = 1 + Math.floor($nodeGraphTransform.scale - 0.5 + 0.001) / 2;

	// Close the context menu when the graph view overlay is closed
	$: if (!$documentState.graphViewOverlayOpen) closeContextMenu();

	let inputElement: HTMLInputElement;

	// Whether a pointer button is held, in which case a drag may be underway and hover hints would only get in the way
	let pointerHeld = false;

	let graph: HTMLDivElement | undefined;

	// The connector the wire being dragged comes from, which stays undimmed, and the one it has snapped onto, which looks hovered
	let wireSourceConnector: Element | undefined;
	let wireTargetConnector: Element | undefined;
	let wireEndsMarked = "";
	$: markWireEndConnectors($nodeGraph.wireInProgressDraggedFrom, $nodeGraph.wireInProgressSnappedTo);

	// The connector a press at the pointer would grab, as the backend decides since it knows the click targets
	let hoveredConnector: Element | undefined;
	let hoveredConnectorMarked = "";
	$: markHoveredConnector($nodeGraph.hoveredConnector);

	// The import, export, or layer with its name being edited in place, and the name the field started with
	type NameTarget = { kind: "Import" | "Export"; index: number } | { kind: "Layer"; nodeId: bigint };
	let editingName: (NameTarget & { initialText: string }) | undefined = undefined;
	let editingNameText = "";

	async function startEditingName(target: NameTarget, currentName: string) {
		editingName = { ...target, initialText: currentName };
		editingNameText = currentName;

		await tick();
		inputElement?.focus();
		inputElement?.select();
	}

	// An unchanged name isn't saved, since the field starts with what's displayed, which may be a fallback like the type
	function commitEditingName() {
		const target = editingName;
		editingName = undefined;
		if (!target || editingNameText === target.initialText) return;

		if (target.kind === "Layer") editor.setLayerNameInGraph(target.nodeId, editingNameText);
		else if (target.kind === "Import") editor.setImportName(target.index, editingNameText);
		else editor.setExportName(target.index, editingNameText);
	}

	// The name fields stop propagation of the keys they handle so no window listener also acts on them
	function onNameInputKeyDown(e: KeyboardEvent) {
		if (e.key === "Enter") commitEditingName();
		else if (e.key === "Escape") editingName = undefined;
		else return;

		e.stopPropagation();
	}

	// The import or export with its description open for editing in a popover pointing at `anchor`, in graph space
	let editingDescription: { isImport: boolean; index: number; anchor: [number, number]; text: string; committedText: string } | undefined = undefined;
	let descriptionInput: TextAreaInput | undefined;

	async function editDescription(isImport: boolean, index: number) {
		const importsExports = $nodeGraphImportsExports;
		if (!importsExports) return;

		const connector = isImport ? importsExports.imports[index] : importsExports.exports[index];
		const description = connector?.description || "";

		// Just beside the connector, on the side facing into the graph
		const [x, y] = isImport ? importsExports.importPosition : importsExports.exportPosition;
		const anchor: [number, number] = [Number(x) + (isImport ? 7 : -5), Number(y) + importExportRowOffset(index) * GRID_SIZE];

		editingDescription = { isImport, index, anchor, text: description, committedText: description };

		await tick();
		descriptionInput?.focus();
	}

	function commitDescription() {
		if (!editingDescription || editingDescription.text === editingDescription.committedText) return;

		if (editingDescription.isImport) editor.setImportDescription(editingDescription.index, editingDescription.text);
		else editor.setExportDescription(editingDescription.index, editingDescription.text);
		editingDescription.committedText = editingDescription.text;
	}

	// How many grid cells below the first import or export the one at `index` sits, with a cell of space separating the primary from the secondary ones
	function importExportRowOffset(index: number): number {
		return index === 0 ? 0 : index + 1;
	}

	// The top of the drop line, at the top of the row at `index`, measured from the first row's center
	function reorderLineTop(index: number): number {
		return importExportRowOffset(index) * GRID_SIZE - GRID_SIZE / 2;
	}

	function closeDescriptionPopover() {
		commitDescription();
		editingDescription = undefined;
	}

	// Ctrl/Cmd+Enter commits the description like in any text area, and here it also closes the popover
	function onDescriptionKeyDown(e: KeyboardEvent) {
		const commandKey = operatingSystem() === "Mac" ? e.metaKey : e.ctrlKey;
		if (commandKey && e.key === "Enter") closeDescriptionPopover();
	}

	// Finds the connectors at the wire's ends, which only change when the wire snaps onto or off of one
	function markWireEndConnectors(draggedFrom: [number, number] | undefined, snappedTo: [number, number] | undefined) {
		const ends = JSON.stringify([draggedFrom, snappedTo]);
		if (ends === wireEndsMarked) return;
		wireEndsMarked = ends;

		wireSourceConnector?.classList.remove("wire-source");
		wireTargetConnector?.classList.remove("wire-target");

		wireSourceConnector = connectorAt(draggedFrom);
		wireTargetConnector = connectorAt(snappedTo);

		wireSourceConnector?.classList.add("wire-source");
		wireTargetConnector?.classList.add("wire-target");
	}

	// The backend picks the hovered connector, so it lights up anywhere in its click target rather than just over its drawn arrow
	function markHoveredConnector(center: [number, number] | undefined) {
		const marked = JSON.stringify(center);
		if (marked === hoveredConnectorMarked) return;
		hoveredConnectorMarked = marked;

		hoveredConnector?.classList.remove("hovered");
		hoveredConnector = connectorAt(center);
		hoveredConnector?.classList.add("hovered");
	}

	// The connector drawn at a point in node graph space, which can sit a few pixels from where its wires attach
	function connectorAt(point: [number, number] | undefined): Element | undefined {
		if (!graph || !point) return undefined;

		const graphBounds = graph.getBoundingClientRect();
		const { x, y, scale } = $nodeGraphTransform;
		return Array.from(graph.querySelectorAll("[data-connector]")).find((connector) => {
			const bounds = connector.getBoundingClientRect();
			const centerX = (bounds.left + bounds.width / 2 - graphBounds.left - x) / scale;
			const centerY = (bounds.top + bounds.height / 2 - graphBounds.top - y) / scale;
			return Math.abs(centerX - point[0]) <= 4 && Math.abs(centerY - point[1]) <= 4;
		});
	}

	function onWindowPointerDown() {
		pointerHeld = true;
	}

	function onWindowPointerUp() {
		pointerHeld = false;
	}

	onMount(() => {
		// Backend dispatches this when the user double-clicks a layer's name area
		subscriptions.subscribeFrontendMessage("TriggerEditLayerNameInGraph", (data) => {
			const node = $nodeGraph.nodes.get(data.nodeId);
			if (node) startEditingName({ kind: "Layer", nodeId: data.nodeId }, node.displayName);
		});

		// Capturing so the state follows the pointer even where another handler stops the event
		window.addEventListener("pointerdown", onWindowPointerDown, true);
		window.addEventListener("pointerup", onWindowPointerUp, true);
		window.addEventListener("pointercancel", onWindowPointerUp, true);
		window.addEventListener("blur", onWindowPointerUp);
	});

	onDestroy(() => {
		subscriptions.unsubscribeFrontendMessage("TriggerEditLayerNameInGraph");

		window.removeEventListener("pointerdown", onWindowPointerDown, true);
		window.removeEventListener("pointerup", onWindowPointerUp, true);
		window.removeEventListener("pointercancel", onWindowPointerUp, true);
		window.removeEventListener("blur", onWindowPointerUp);
	});

	function calculateGridSpacing(scale: number): number {
		const dense = scale * GRID_SIZE;
		let sparse = dense;

		while (sparse > 0 && sparse < GRID_COLLAPSE_SPACING) {
			sparse *= 2;
		}

		return sparse;
	}

	function createNode(identifier: string) {
		if ($nodeGraph.contextMenuInformation === undefined) return;

		editor.createNode(identifier, $nodeGraph.contextMenuInformation.nodeCreationCoordinates[0], $nodeGraph.contextMenuInformation.nodeCreationCoordinates[1]);
	}

	function nodeBorderMask(nodeWidth: number, primaryInputExists: boolean, exposedSecondaryInputs: number, primaryOutputExists: boolean, exposedSecondaryOutputs: number): string {
		const nodeHeight = Math.max(1 + exposedSecondaryInputs, 1 + exposedSecondaryOutputs) * 24;

		const boxes: { x: number; y: number; width: number; height: number }[] = [];

		// Primary input
		if (primaryInputExists) boxes.push({ x: -8, y: 4, width: 16, height: 16 });
		// Secondary inputs
		for (let i = 0; i < exposedSecondaryInputs; i++) boxes.push({ x: -8, y: 4 + (i + 1) * 24, width: 16, height: 16 });

		// Primary output
		if (primaryOutputExists) boxes.push({ x: nodeWidth - 8, y: 4, width: 16, height: 16 });
		// Exposed outputs
		for (let i = 0; i < exposedSecondaryOutputs; i++) boxes.push({ x: nodeWidth - 8, y: 4 + (i + 1) * 24, width: 16, height: 16 });

		return borderMask(boxes, nodeWidth, nodeHeight);
	}

	function layerBorderMask(nodeWidthFromThumbnail: number, nodeChainAreaLeftExtension: number, hasLeftInputWire: boolean): string {
		const NODE_HEIGHT = 2 * 24;
		const THUMBNAIL_WIDTH = 72 + 8 * 2;
		const FUDGE_HEIGHT_BEYOND_LAYER_HEIGHT = 2;

		const nodeWidth = nodeWidthFromThumbnail + nodeChainAreaLeftExtension;

		const boxes: { x: number; y: number; width: number; height: number }[] = [];

		// Left input
		if (hasLeftInputWire && nodeChainAreaLeftExtension > 0) {
			boxes.push({ x: -8, y: 16, width: 16, height: 16 });
		}

		// Thumbnail
		boxes.push({ x: nodeChainAreaLeftExtension - 8, y: -FUDGE_HEIGHT_BEYOND_LAYER_HEIGHT, width: THUMBNAIL_WIDTH, height: NODE_HEIGHT + FUDGE_HEIGHT_BEYOND_LAYER_HEIGHT * 2 });

		// Right visibility button
		boxes.push({ x: nodeWidth - 12, y: (NODE_HEIGHT - 24) / 2, width: 24, height: 24 });

		return borderMask(boxes, nodeWidth, NODE_HEIGHT);
	}

	function borderMask(boxes: { x: number; y: number; width: number; height: number }[], nodeWidth: number, nodeHeight: number): string {
		const rectangles = boxes.map((box) => `M${box.x},${box.y} L${box.x + box.width},${box.y} L${box.x + box.width},${box.y + box.height} L${box.x},${box.y + box.height}z`);
		return `M-2,-2 L${nodeWidth + 2},-2 L${nodeWidth + 2},${nodeHeight + 2} L-2,${nodeHeight + 2}z ${rectangles.join(" ")}`;
	}

	// Whether the wire feeding an input is the one a dragged node would be inserted onto if dropped now
	function isInsertionPreview(preview: [bigint, number] | undefined, nodeId: bigint, inputIndex: number): boolean {
		return preview !== undefined && preview[0] === nodeId && preview[1] === inputIndex;
	}

	// Whatever is gliding into the place it moved to is drawn short of that place, by this many grid cells along the axis.
	// It shifts the position rather than transforming, since a transform would trap a layer's connectors beneath the wires drawn after it.
	function glideCells(offset: [number, number] | undefined, axis: 0 | 1): number {
		return (offset?.[axis] || 0) / GRID_SIZE;
	}

	function nodeNameTooltipLabel(node: FrontendNode): string {
		return node.displayName === node.implementationName ? node.displayName : `${node.implementationName} (${node.displayName})`;
	}

	function codeList(types: string[]): string {
		return types.map((type) => `\`${type}\``).join("\n");
	}

	function connectorTooltipLabel(connector: FrontendGraphInput | FrontendGraphOutput): string {
		return connector.name || connector.resolvedType;
	}

	// The connector's description, then below a divider, the type of its wire (when it has one) and the types it could have
	function connectorTooltipDescription(connector: FrontendGraphInput | FrontendGraphOutput, possibleTypes?: string[]): string {
		const wiredType = connector.connected ? `Wired Type:\n\`${connector.resolvedType}\`` : "";
		const possibleTypesText = possibleTypes ? `Possible Types:\n${possibleTypes.length > 0 ? codeList(possibleTypes) : "None"}` : "";
		const types = [wiredType, possibleTypesText].filter(Boolean).join("\n\n");

		return [connector.description.trim(), types].filter(Boolean).join("\n---\n");
	}

	function inputTooltipDescription(input: FrontendGraphInput): string {
		return connectorTooltipDescription(input, input.validTypes);
	}

	// Unlike a node input's, the possible types of an export or output are left out where they aren't known
	function exportTooltipDescription(exportConnector: FrontendGraphInput): string {
		return connectorTooltipDescription(exportConnector, exportConnector.validTypes.length > 0 ? exportConnector.validTypes : undefined);
	}

	function outputTooltipDescription(output: FrontendGraphOutput): string {
		return connectorTooltipDescription(output, output.possibleTypes.length > 0 ? output.possibleTypes : undefined);
	}

	function zipWithUndefined(arr1: FrontendGraphInput[], arr2: FrontendGraphOutput[]) {
		const maxLength = Math.max(arr1.length, arr2.length);
		const result = [];
		for (let i = 0; i < maxLength; i++) {
			result.push([arr1[i], arr2[i]]);
		}
		return result;
	}
</script>

<div class="graph" class:pointer-held={pointerHeld} bind:this={graph} data-node-graph>
	<div
		class="grid-background"
		style:--grid-spacing={`${gridSpacing}px`}
		style:--grid-offset-x={`${$nodeGraphTransform.x}px`}
		style:--grid-offset-y={`${$nodeGraphTransform.y}px`}
		style:--grid-dot-radius={`${gridDotRadius}px`}
	></div>
	<!-- Right click menu for adding nodes -->
	{#if $nodeGraph.contextMenuInformation}
		<FloatingMenu
			class="context-menu"
			data-context-menu
			styles={{
				left: `${$nodeGraph.contextMenuInformation.contextMenuCoordinates[0] * $nodeGraphTransform.scale + $nodeGraphTransform.x}px`,
				top: `${$nodeGraph.contextMenuInformation.contextMenuCoordinates[1] * $nodeGraphTransform.scale + $nodeGraphTransform.y}px`,
			}}
			open={true}
			type="Popover"
			direction="BottomLeft"
		>
			{#if $nodeGraph.contextMenuInformation.contextMenuData.type === "CreateNode"}
				<NodeCatalog initialSearchTerm={$nodeGraph.contextMenuInformation.contextMenuData.data.compatibleType || ""} on:selectNodeType={(e) => createNode(e.detail)} />
			{:else if $nodeGraph.contextMenuInformation.contextMenuData.type === "ModifyNode"}
				<LayoutCol class="modify-node-menu">
					<TextButton
						label="Merge Selected Nodes"
						action={() => {
							editor.mergeSelectedNodes();
							closeContextMenu();
						}}
						flush={true}
					/>
					{@const currentlyIsNode = $nodeGraph.contextMenuInformation.contextMenuData.data.currentlyIsNode}
					<TextButton
						label={currentlyIsNode ? "Display as Layer" : "Display as Node"}
						action={() => {
							if ($nodeGraph.contextMenuInformation?.contextMenuData.type === "ModifyNode") {
								editor.setToNodeOrLayer($nodeGraph.contextMenuInformation.contextMenuData.data.nodeId, currentlyIsNode);
							}
							closeContextMenu();
						}}
						disabled={!$nodeGraph.contextMenuInformation.contextMenuData.data.canBeLayer}
						flush={true}
					/>
					{#if $nodeGraph.contextMenuInformation.contextMenuData.data.hasSelectedLayers}
						{@const allLocked = $nodeGraph.contextMenuInformation.contextMenuData.data.allSelectedLayersLocked}
						{@const nodeId = $nodeGraph.contextMenuInformation.contextMenuData.data.nodeId}
						<TextButton
							label={allLocked ? "Unlock" : "Lock"}
							action={() => {
								if ($nodeGraph.selected.includes(nodeId)) {
									editor.toggleSelectedLocked();
								} else {
									editor.toggleLayerLock(nodeId);
								}
								closeContextMenu();
							}}
							flush={true}
						/>
					{/if}
				</LayoutCol>
			{:else if $nodeGraph.contextMenuInformation.contextMenuData.type === "ModifyImportExport"}
				{@const isImport = $nodeGraph.contextMenuInformation.contextMenuData.data.isImport}
				{@const index = $nodeGraph.contextMenuInformation.contextMenuData.data.index}
				{@const exposed = $nodeGraph.contextMenuInformation.contextMenuData.data.exposed}
				<LayoutCol class="modify-node-menu">
					<TextButton
						label="Rename"
						action={() => {
							const currentName = (isImport ? $nodeGraphImportsExports?.imports[index]?.name : $nodeGraphImportsExports?.exports[index]?.name) || "";
							startEditingName({ kind: isImport ? "Import" : "Export", index }, currentName);
							closeContextMenu();
						}}
						flush={true}
					/>
					<TextButton
						label="Edit Description"
						action={() => {
							editDescription(isImport, index);
							closeContextMenu();
						}}
						flush={true}
					/>
					{#if index > 0}
						<TextButton
							label="Make Primary"
							action={() => {
								if (isImport) editor.makeImportPrimary(index);
								else editor.makeExportPrimary(index);
								closeContextMenu();
							}}
							flush={true}
						/>
					{:else}
						<TextButton
							label="Make Secondary"
							action={() => {
								if (isImport) editor.makeImportSecondary();
								else editor.makeExportSecondary();
								closeContextMenu();
							}}
							flush={true}
						/>
					{/if}
					{#if isImport && index > 0}
						<TextButton
							label={exposed ? "Unexpose in Graph" : "Expose in Graph"}
							action={() => {
								editor.setImportExposed(index, !exposed);
								closeContextMenu();
							}}
							flush={true}
						/>
					{/if}
					<TextButton
						label="Disconnect"
						action={() => {
							if (isImport) editor.disconnectImport(index);
							else editor.disconnectExport(index);
							closeContextMenu();
						}}
						disabled={!$nodeGraph.contextMenuInformation.contextMenuData.data.connected}
						flush={true}
					/>
					<TextButton
						label="Remove"
						action={() => {
							if (isImport) editor.removeImport(index);
							else editor.removeExport(index);
							closeContextMenu();
						}}
						flush={true}
					/>
				</LayoutCol>
			{/if}
		</FloatingMenu>
	{/if}

	<!-- Popover for editing the description of an import or export, kept mounted so opening it attaches its listeners for closing -->
	<FloatingMenu
		class="description-popover"
		styles={editingDescription
			? {
					left: `${editingDescription.anchor[0] * $nodeGraphTransform.scale + $nodeGraphTransform.x}px`,
					top: `${editingDescription.anchor[1] * $nodeGraphTransform.scale + $nodeGraphTransform.y}px`,
				}
			: {}}
		open={editingDescription !== undefined}
		on:open={({ detail }) => {
			if (!detail) closeDescriptionPopover();
		}}
		type="Popover"
		direction={editingDescription?.isImport ? "Right" : "Left"}
		strayCloses={false}
	>
		{#if editingDescription}
			<TextAreaInput
				value={editingDescription.committedText}
				on:value={({ detail }) => {
					if (editingDescription) editingDescription.text = detail;
				}}
				on:commitText={commitDescription}
				on:keydown={onDescriptionKeyDown}
				bind:this={descriptionInput}
			/>
		{/if}
	</FloatingMenu>

	{#if $nodeGraph.errors.length > 0}
		<div class="node-error-container" style:transform-origin="0 0" style:transform={`translate(${$nodeGraphTransform.x}px, ${$nodeGraphTransform.y}px) scale(${$nodeGraphTransform.scale})`}>
			{#each $nodeGraph.errors as error}
				{@const glideOffset = $nodeGlideOffsets.nodes.get(error.nodeId)}
				{@const left = `${error.position[0] + (glideOffset?.[0] || 0)}px`}
				{@const top = `${error.position[1] + (glideOffset?.[1] || 0)}px`}
				<span class="node-error faded" style:left style:top transition:fade={FADE_TRANSITION}>
					{error.error}
				</span>
				<span class="node-error hover" style:left style:top transition:fade={FADE_TRANSITION}>
					{error.error}
				</span>
			{/each}
		</div>
	{/if}

	<!-- Click target debug visualizations -->
	{#if $nodeGraph.clickTargets}
		<div class="click-targets" style:transform-origin="0 0" style:transform={`translate(${$nodeGraphTransform.x}px, ${$nodeGraphTransform.y}px) scale(${$nodeGraphTransform.scale})`}>
			<svg>
				{#each $nodeGraph.clickTargets.nodeClickTargets as pathString}
					<path class="node" d={pathString} />
				{/each}
				{#each $nodeGraph.clickTargets.layerClickTargets as pathString}
					<path class="layer" d={pathString} />
				{/each}
				{#each $nodeGraph.clickTargets.connectorClickTargets as pathString}
					<path class="connector" d={pathString} />
				{/each}
				{#each $nodeGraph.clickTargets.iconClickTargets as pathString}
					<path class="visibility" d={pathString} />
				{/each}
				<path class="all-nodes-bounding-box" d={$nodeGraph.clickTargets.allNodesBoundingBox} />
				{#each $nodeGraph.clickTargets.importExportNames as pathString}
					<path class="import-export-name" d={pathString} />
				{/each}
			</svg>
		</div>
	{/if}

	<!-- Thick vertical layer connection wires -->
	<div class="wires" style:transform-origin="0 0" style:transform={`translate(${$nodeGraphTransform.x}px, ${$nodeGraphTransform.y}px) scale(${$nodeGraphTransform.scale})`}>
		<svg>
			{#each $nodeGraphWires.entries() as [nodeId, map]}
				{#each map.entries() as [inputIndex, { pathString, centerPathString, dataType, thick, dashed }]}
					{#if thick}
						<g class:dashed class:undimmed={isInsertionPreview($wireInsertionPreview, nodeId, inputIndex)}>
							<path
								d={pathString}
								style:--data-line-width="8px"
								style:--data-color={`var(--color-data-${dataType.toLowerCase()})`}
								style:--data-color-dim={`var(--color-data-${dataType.toLowerCase()}-dim)`}
								style:--data-dasharray={`3,${dashed ? 2 : 0}`}
							/>
							<!-- A thin inner line splits the layer-stack wire down the middle reaching past the ends into the cleaved connector slots -->
							<path d={centerPathString} style:--data-line-width="2px" style:--data-color="#444444" style:--data-color-dim="#444444" style:--data-dasharray={`3,${dashed ? 2 : 0}`} />
						</g>
					{/if}
				{/each}
			{/each}
		</svg>
	</div>

	<!-- Import and Export connectors -->
	<div class="imports-and-exports" style:transform-origin="0 0" style:transform={`translate(${$nodeGraphTransform.x}px, ${$nodeGraphTransform.y}px) scale(${$nodeGraphTransform.scale})`}>
		{#if $nodeGraphImportsExports}
			{#each $nodeGraphImportsExports.imports as frontendOutput, index}
				{#if frontendOutput}
					<svg
						xmlns="http://www.w3.org/2000/svg"
						viewBox="0 0 8 8"
						class="connector"
						data-connector="output"
						data-tooltip-label={connectorTooltipLabel(frontendOutput)}
						data-tooltip-description={outputTooltipDescription(frontendOutput)}
						data-datatype={frontendOutput.dataType}
						style:--data-color={`var(--color-data-${frontendOutput.dataType.toLowerCase()})`}
						style:--data-color-dim={`var(--color-data-${frontendOutput.dataType.toLowerCase()}-dim)`}
						style:--offset-left={($nodeGraphImportsExports.importPosition[0] - 8) / 24 + glideCells($nodeGlideOffsets.imports, 0)}
						style:--offset-top={($nodeGraphImportsExports.importPosition[1] - 8) / 24 + glideCells($nodeGlideOffsets.imports, 1) + importExportRowOffset(index)}
					>
						{#if frontendOutput.connected}
							<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color)" />
						{:else}
							<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color-dim)" />
						{/if}
					</svg>

					<div
						class="edit-import-export import"
						class:separator-bottom={index === 0 && $nodeGraphImportsExports.addImportExport}
						class:separator-top={index === 1 && $nodeGraphImportsExports.addImportExport}
						style:--offset-left={($nodeGraphImportsExports.importPosition[0] - 8) / 24 + glideCells($nodeGlideOffsets.imports, 0)}
						style:--offset-top={($nodeGraphImportsExports.importPosition[1] - 8) / 24 + glideCells($nodeGlideOffsets.imports, 1) + importExportRowOffset(index)}
					>
						{#if editingName?.kind === "Import" && editingName.index === index}
							<input
								class="name-input"
								type="text"
								bind:this={inputElement}
								bind:value={editingNameText}
								on:pointerdown|stopPropagation
								on:dblclick|stopPropagation
								on:blur={commitEditingName}
								on:keydown={onNameInputKeyDown}
							/>
						{:else}
							<p
								class="import-text"
								data-tooltip-label={frontendOutput.name}
								data-tooltip-description={frontendOutput.description}
								on:dblclick={() => startEditingName({ kind: "Import", index }, frontendOutput.name)}
							>
								{frontendOutput.name}
							</p>
						{/if}
					</div>
				{:else if $nodeGraphImportsExports.addImportExport}
					<svg
						xmlns="http://www.w3.org/2000/svg"
						viewBox="0 0 8 8"
						class="connector"
						data-connector="output"
						data-tooltip-label="Add Primary Import"
						data-tooltip-description="Drag a wire from here and connect it to an input, or drop one here from an input."
						style:--data-color="var(--color-data-general)"
						style:--data-color-dim="var(--color-data-general-dim)"
						style:--offset-left={($nodeGraphImportsExports.importPosition[0] - 8) / 24 + glideCells($nodeGlideOffsets.imports, 0)}
						style:--offset-top={($nodeGraphImportsExports.importPosition[1] - 8) / 24 + glideCells($nodeGlideOffsets.imports, 1) + importExportRowOffset(index)}
					>
						<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color-dim)" />
					</svg>
				{/if}
			{/each}

			{#each $nodeGraphImportsExports.exports as frontendInput, index}
				{#if frontendInput}
					<svg
						xmlns="http://www.w3.org/2000/svg"
						viewBox="0 0 8 8"
						class="connector"
						data-connector="input"
						data-tooltip-label={connectorTooltipLabel(frontendInput)}
						data-tooltip-description={exportTooltipDescription(frontendInput)}
						data-datatype={frontendInput.dataType}
						style:--data-color={`var(--color-data-${frontendInput.dataType.toLowerCase()})`}
						style:--data-color-dim={`var(--color-data-${frontendInput.dataType.toLowerCase()}-dim)`}
						style:--offset-left={($nodeGraphImportsExports.exportPosition[0] - 8) / 24 + glideCells($nodeGlideOffsets.exports, 0)}
						style:--offset-top={($nodeGraphImportsExports.exportPosition[1] - 8) / 24 + glideCells($nodeGlideOffsets.exports, 1) + importExportRowOffset(index)}
					>
						{#if frontendInput.connected}
							<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color)" />
						{:else}
							<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color-dim)" />
						{/if}
					</svg>
					<div
						class="edit-import-export export"
						class:separator-bottom={index === 0 && $nodeGraphImportsExports.addImportExport}
						class:separator-top={index === 1 && $nodeGraphImportsExports.addImportExport}
						style:--offset-left={($nodeGraphImportsExports.exportPosition[0] - 8) / 24 + glideCells($nodeGlideOffsets.exports, 0)}
						style:--offset-top={($nodeGraphImportsExports.exportPosition[1] - 8) / 24 + glideCells($nodeGlideOffsets.exports, 1) + importExportRowOffset(index)}
					>
						{#if editingName?.kind === "Export" && editingName.index === index}
							<input
								class="name-input"
								type="text"
								bind:this={inputElement}
								bind:value={editingNameText}
								on:pointerdown|stopPropagation
								on:dblclick|stopPropagation
								on:blur={commitEditingName}
								on:keydown={onNameInputKeyDown}
							/>
						{:else}
							<!-- Only a subgraph's exports have names to edit, unlike the document's Canvas export -->
							<p
								class="export-text"
								data-tooltip-label={frontendInput.name}
								data-tooltip-description={frontendInput.description}
								on:dblclick={() => {
									if ($nodeGraphImportsExports?.addImportExport) startEditingName({ kind: "Export", index }, frontendInput.name);
								}}
							>
								{frontendInput.name}
							</p>
						{/if}
					</div>
				{:else if $nodeGraphImportsExports.addImportExport}
					<svg
						xmlns="http://www.w3.org/2000/svg"
						viewBox="0 0 8 8"
						class="connector"
						data-connector="input"
						data-tooltip-label="Add Primary Export"
						data-tooltip-description="Drag a wire from here and connect it to an output, or drop one here from an output."
						style:--data-color="var(--color-data-general)"
						style:--data-color-dim="var(--color-data-general-dim)"
						style:--offset-left={($nodeGraphImportsExports.exportPosition[0] - 8) / 24 + glideCells($nodeGlideOffsets.exports, 0)}
						style:--offset-top={($nodeGraphImportsExports.exportPosition[1] - 8) / 24 + glideCells($nodeGlideOffsets.exports, 1) + importExportRowOffset(index)}
					>
						<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color-dim)" />
					</svg>
				{/if}
			{/each}

			<!-- Connectors for adding an import or export, an extra grid cell below the last so they aren't mistaken for unnamed ones -->
			{#if $nodeGraphImportsExports.addImportExport}
				<svg
					xmlns="http://www.w3.org/2000/svg"
					viewBox="0 0 8 8"
					class="connector"
					data-connector="output"
					data-tooltip-label="Add Import"
					data-tooltip-description="Drag a wire from here and connect it to an input, or drop one here from an input."
					style:--data-color="var(--color-data-general)"
					style:--data-color-dim="var(--color-data-general-dim)"
					style:--offset-left={($nodeGraphImportsExports.importPosition[0] - 8) / 24 + glideCells($nodeGlideOffsets.imports, 0)}
					style:--offset-top={($nodeGraphImportsExports.importPosition[1] - 8) / 24 +
						glideCells($nodeGlideOffsets.imports, 1) +
						importExportRowOffset($nodeGraphImportsExports.imports.length) +
						1}
				>
					<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color-dim)" />
				</svg>
				<svg
					xmlns="http://www.w3.org/2000/svg"
					viewBox="0 0 8 8"
					class="connector"
					data-connector="input"
					data-tooltip-label="Add Export"
					data-tooltip-description="Drag a wire from here and connect it to an output, or drop one here from an output."
					style:--data-color="var(--color-data-general)"
					style:--data-color-dim="var(--color-data-general-dim)"
					style:--offset-left={($nodeGraphImportsExports.exportPosition[0] - 8) / 24 + glideCells($nodeGlideOffsets.exports, 0)}
					style:--offset-top={($nodeGraphImportsExports.exportPosition[1] - 8) / 24 +
						glideCells($nodeGlideOffsets.exports, 1) +
						importExportRowOffset($nodeGraphImportsExports.exports.length) +
						1}
				>
					<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color-dim)" />
				</svg>
			{/if}

			{#if $nodeGraph.reorderImportIndex !== undefined}
				{@const position = {
					x: Number($nodeGraphImportsExports.importPosition[0]),
					y: Number($nodeGraphImportsExports.importPosition[1]) + reorderLineTop(Number($nodeGraph.reorderImportIndex)),
				}}
				<div
					class="reorder-bar"
					style:--offset-left={(position.x - 58) / 24 + glideCells($nodeGlideOffsets.imports, 0)}
					style:--offset-top={position.y / 24 + glideCells($nodeGlideOffsets.imports, 1)}
				></div>
			{/if}

			{#if $nodeGraph.reorderExportIndex !== undefined}
				{@const position = {
					x: Number($nodeGraphImportsExports.exportPosition[0]),
					y: Number($nodeGraphImportsExports.exportPosition[1]) + reorderLineTop(Number($nodeGraph.reorderExportIndex)),
				}}
				<div
					class="reorder-bar"
					style:--offset-left={(position.x + 9) / 24 + glideCells($nodeGlideOffsets.exports, 0)}
					style:--offset-top={position.y / 24 + glideCells($nodeGlideOffsets.exports, 1)}
				></div>
			{/if}
		{/if}
	</div>

	<!-- Layers and nodes -->
	<div class="layers-and-nodes" style:transform-origin="0 0" style:transform={`translate(${$nodeGraphTransform.x}px, ${$nodeGraphTransform.y}px) scale(${$nodeGraphTransform.scale})`}>
		<!-- Layers -->
		{#each Array.from($nodeGraph.nodes)
			.filter(([nodeId, node]) => node.isLayer && $visibleNodes.has(nodeId))
			.map(([_, node]) => node) as node (node.id)}
			{@const clipPathId = String(Math.random()).substring(2)}
			{@const stackDataInput = node.exposedInputs[0]}
			{@const layerAreaWidth = $nodeGraph.layerWidths.get(node.id) || 8}
			{@const layerChainWidth = $nodeGraph.chainWidths.get(node.id) || 0}
			{@const hasLeftInputWire = $nodeGraph.hasLeftInputWire.get(node.id) || false}
			{@const description = node.reference ? $nodeGraph.nodeDescriptions.get(node.reference) : undefined}
			<div
				class="layer"
				class:selected={$nodeGraph.selected.includes(node.id)}
				class:in-selected-network={$nodeGraph.inSelectedNetwork}
				class:previewed={node.previewed}
				class:disabled={!node.visible}
				class:locked={node.locked}
				style:--offset-left={(node.position?.[0] || 0) + glideCells($nodeGlideOffsets.nodes.get(node.id), 0)}
				style:--offset-top={(node.position?.[1] || 0) + glideCells($nodeGlideOffsets.nodes.get(node.id), 1)}
				style:--clip-path-id={`url(#${clipPathId})`}
				style:--data-color={`var(--color-data-${(node.primaryOutput?.dataType || "General").toLowerCase()})`}
				style:--data-color-dim={`var(--color-data-${(node.primaryOutput?.dataType || "General").toLowerCase()}-dim)`}
				style:--layer-area-width={layerAreaWidth}
				style:--node-chain-area-left-extension={layerChainWidth !== 0 ? layerChainWidth + 0.5 : 0}
				data-tooltip-label={nodeNameTooltipLabel(node)}
				data-tooltip-description={(description || "").trim()}
				data-node={node.id}
			>
				<div class="thumbnail">
					{#if $nodeGraph.thumbnails.has(node.id)}
						{@html $nodeGraph.thumbnails.get(node.id)}
					{/if}
					<!-- Layer stacking top output -->
					{#if node.primaryOutput}
						<svg
							xmlns="http://www.w3.org/2000/svg"
							viewBox="0 0 8 12"
							class="connector top"
							data-connector="output"
							data-tooltip-label={connectorTooltipLabel(node.primaryOutput)}
							data-tooltip-description={outputTooltipDescription(node.primaryOutput)}
							data-datatype={node.primaryOutput.dataType}
							style:--data-color={`var(--color-data-${node.primaryOutput.dataType.toLowerCase()})`}
							style:--data-color-dim={`var(--color-data-${node.primaryOutput.dataType.toLowerCase()}-dim)`}
						>
							{#if node.primaryOutput.connected}
								<path d="M0,6.953l2.521,-1.694a2.649,2.649,0,0,1,2.959,0l2.52,1.694v5.047h-8z" fill="var(--data-color)" />
								{#if node.primaryOutputConnectedToLayer}
									<path
										class="stack-wire-end"
										d="M0,4.5 C0,4.5 0,-3.5 0,-3.5 C0,-3.5 3,-3.5 3,-3.5 C3,-3.5 3,2.565 3,2.565 C2.834,2.632 2.673,2.717 2.52,2.819 C2.52,2.819 0,4.5 0,4.5 ZM5,2.566 C5,2.566 5,-3.5 5,-3.5 C5,-3.5 8,-3.5 8,-3.5 C8,-3.5 8,4.5 8,4.5 C8,4.5 5.479,2.819 5.479,2.819 C5.326,2.717 5.166,2.633 5,2.566 Z"
										fill="var(--data-color-dim)"
									/>
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
						class="connector bottom"
						data-connector="input"
						data-tooltip-label={node.primaryInput ? connectorTooltipLabel(node.primaryInput) : ""}
						data-tooltip-description={node.primaryInput ? inputTooltipDescription(node.primaryInput) : ""}
						data-datatype={node.primaryInput?.dataType}
						style:--data-color={`var(--color-data-${(node.primaryInput?.dataType || "General").toLowerCase()})`}
						style:--data-color-dim={`var(--color-data-${(node.primaryInput?.dataType || "General").toLowerCase()}-dim)`}
					>
						{#if node.primaryInput?.connected}
							<path d="M0,0H8V8L5.479,6.319a2.666,2.666,0,0,0-2.959,0L0,8Z" fill="var(--data-color)" />
							{#if node.primaryInputConnectedToLayer}
								<path
									class="stack-wire-end"
									d="M2.512,9.26 C2.673,9.157 2.834,9.072 3,9 C3,9 3,16 3,16 C3,16 0,16 0,16 C0,16 0,10.95 0,10.95 C0,10.95 2.512,9.26 2.512,9.26 ZM5,16 C5,16 5,9 5,9 C5.166,9.073 5.327,9.158 5.48,9.26 C5.48,9.26 8,10.95 8,10.95 C8,10.95 8,16 8,16 C8,16 5,16 5,16 Z"
									fill="var(--data-color-dim)"
								/>
							{/if}
						{:else}
							<path d="M0,0H8V8L5.479,6.319a2.666,2.666,0,0,0-2.959,0L0,8Z" fill="var(--data-color-dim)" />
						{/if}
					</svg>
				</div>
				<!-- Layer input connector (from left) -->
				{#if node.exposedInputs.length > 0}
					<div class="input connectors">
						<svg
							xmlns="http://www.w3.org/2000/svg"
							viewBox="0 0 8 8"
							class="connector"
							data-tooltip-label={connectorTooltipLabel(stackDataInput)}
							data-tooltip-description={inputTooltipDescription(stackDataInput)}
							data-connector="input"
							data-datatype={stackDataInput.dataType}
							style:--data-color={`var(--color-data-${stackDataInput.dataType.toLowerCase()})`}
							style:--data-color-dim={`var(--color-data-${stackDataInput.dataType.toLowerCase()}-dim)`}
						>
							{#if stackDataInput.connected}
								<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color)" />
							{:else}
								<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color-dim)" />
							{/if}
						</svg>
					</div>
				{/if}
				<div class="details">
					{#if editingName?.kind === "Layer" && editingName.nodeId === node.id}
						<input
							class="name-input"
							type="text"
							bind:this={inputElement}
							bind:value={editingNameText}
							on:pointerdown|stopPropagation
							on:dblclick|stopPropagation
							on:blur={commitEditingName}
							on:keydown={onNameInputKeyDown}
						/>
					{:else}
						<TextLabel>{node.displayName}</TextLabel>
					{/if}
				</div>
				<div class="solo-drag-grip" data-tooltip-description="Drag only this layer without pushing others outside the stack"></div>
				{#if node.locked}
					<IconButton
						class="lock"
						data-lock-button
						size={24}
						icon="PadlockLocked"
						hoverIcon="PadlockUnlocked"
						action={() => {
							/* Button is purely visual, clicking is handled in NodeGraphMessage::PointerDown */
						}}
						tooltipLabel="Unlock"
					/>
				{/if}
				<IconButton
					class="visibility"
					data-visibility-button
					size={24}
					icon={node.visible ? "EyeVisible" : "EyeHidden"}
					hoverIcon={node.visible ? "EyeHide" : "EyeShow"}
					action={() => {
						/* Button is purely visual, clicking is handled in NodeGraphMessage::PointerDown */
					}}
					tooltipLabel={node.visible ? "Hide" : "Show"}
				/>

				<svg class="border-mask" width="0" height="0">
					<defs>
						<clipPath id={clipPathId}>
							<!-- Keep this equation in sync with the equivalent one in the CSS rule for `.layer { width: ... }` below -->
							<path clip-rule="evenodd" d={layerBorderMask(24 * layerAreaWidth - 12, layerChainWidth ? (0.5 + layerChainWidth) * 24 : 0, hasLeftInputWire)} />
						</clipPath>
					</defs>
				</svg>
			</div>
		{/each}

		<!-- Node connection wires -->
		<div class="wires">
			<svg>
				{#each $nodeGraphWires.entries() as [nodeId, map]}
					{#each map.entries() as [inputIndex, { pathString, dataType, thick, dashed, isList }]}
						{#if !thick}
							<g class:dashed class:undimmed={isInsertionPreview($wireInsertionPreview, nodeId, inputIndex)}>
								{#if isList}
									<!-- A rank-1 List wire reads as two parallel lines: a triple-width data line split down the middle by a 1x background-colored overlay -->
									<path
										d={pathString}
										style:--data-line-width="4px"
										style:--data-color={`var(--color-data-${dataType.toLowerCase()})`}
										style:--data-color-dim={`var(--color-data-${dataType.toLowerCase()}-dim)`}
										style:--data-dasharray={`3,${dashed ? 2 : 0}`}
									/>
									<path d={pathString} style:--data-line-width="2px" style:--data-color="#444444" style:--data-color-dim="#444444" style:--data-dasharray={`3,${dashed ? 2 : 0}`} />
								{:else}
									<path
										d={pathString}
										style:--data-line-width="2px"
										style:--data-color={`var(--color-data-${dataType.toLowerCase()})`}
										style:--data-color-dim={`var(--color-data-${dataType.toLowerCase()}-dim)`}
										style:--data-dasharray={`3,${dashed ? 2 : 0}`}
									/>
								{/if}
							</g>
						{/if}
					{/each}
				{/each}
				{#if $nodeGraph.wirePathInProgress}
					<path
						d={$nodeGraph.wirePathInProgress?.pathString}
						style:--data-line-width={`${$nodeGraph.wirePathInProgress.thick ? 8 : 2}px`}
						style:--data-color={`var(--color-data-${$nodeGraph.wirePathInProgress.dataType.toLowerCase()})`}
						style:--data-color-dim={`var(--color-data-${$nodeGraph.wirePathInProgress.dataType.toLowerCase()}-dim)`}
						style:--data-dasharray={`3,${$nodeGraph.wirePathInProgress.dashed ? 2 : 0}`}
					/>
				{/if}
			</svg>
		</div>

		<!-- Nodes -->
		{#each Array.from($nodeGraph.nodes)
			.filter(([nodeId, node]) => !node.isLayer && !node.isRedirectDot && $visibleNodes.has(nodeId))
			.map(([_, node]) => node) as node (node.id)}
			{@const exposedInputsOutputs = zipWithUndefined(node.exposedInputs, node.exposedOutputs)}
			{@const clipPathId = String(Math.random()).substring(2)}
			{@const description = node.reference ? $nodeGraph.nodeDescriptions.get(node.reference) : undefined}
			{@const icon = (node.reference && $nodeGraph.nodeIcons.get(node.reference)) || "NodeCustomNode"}
			<div
				class="node"
				class:selected={$nodeGraph.selected.includes(node.id)}
				class:previewed={node.previewed}
				class:disabled={!node.visible}
				style:--offset-left={(node.position?.[0] || 0) + glideCells($nodeGlideOffsets.nodes.get(node.id), 0)}
				style:--offset-top={(node.position?.[1] || 0) + glideCells($nodeGlideOffsets.nodes.get(node.id), 1)}
				style:--clip-path-id={`url(#${clipPathId})`}
				style:--data-color={`var(--color-data-${(node.primaryOutput?.dataType || "General").toLowerCase()})`}
				style:--data-color-dim={`var(--color-data-${(node.primaryOutput?.dataType || "General").toLowerCase()}-dim)`}
				data-tooltip-label={nodeNameTooltipLabel(node)}
				data-tooltip-description={(description || "").trim()}
				data-node={node.id}
			>
				<!-- Primary row -->
				<div class="primary" class:in-selected-network={$nodeGraph.inSelectedNetwork} class:no-secondary-section={exposedInputsOutputs.length === 0}>
					<IconLabel {icon} />
					<!-- TODO: Allow the user to edit the name, just like in the Layers panel -->
					<TextLabel>{node.displayName}</TextLabel>
				</div>
				<!-- Secondary rows -->
				{#if exposedInputsOutputs.length > 0}
					<div class="secondary" class:in-selected-network={$nodeGraph.inSelectedNetwork}>
						{#each exposedInputsOutputs as [input, output]}
							<div class={`secondary-row expanded ${input !== undefined ? "input" : "output"}`}>
								<TextLabel tooltipLabel={input !== undefined ? input.name : output.name} tooltipDescription={input !== undefined ? input.description : output.description}>
									{input !== undefined ? input.name : output.name}
								</TextLabel>
							</div>
						{/each}
					</div>
				{/if}
				<!-- Input connectors -->
				<div class="input connectors">
					{#if node.primaryInput?.dataType}
						<svg
							xmlns="http://www.w3.org/2000/svg"
							viewBox="0 0 8 8"
							class="connector primary-connector"
							data-connector="input"
							data-tooltip-label={connectorTooltipLabel(node.primaryInput)}
							data-tooltip-description={inputTooltipDescription(node.primaryInput)}
							data-datatype={node.primaryInput?.dataType}
							style:--data-color={`var(--color-data-${node.primaryInput.dataType.toLowerCase()})`}
							style:--data-color-dim={`var(--color-data-${node.primaryInput.dataType.toLowerCase()}-dim)`}
						>
							{#if node.primaryInput.connected}
								<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color)" />
							{:else}
								<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color-dim)" />
							{/if}
						</svg>
					{/if}
					{#each node.exposedInputs as secondary, index}
						{#if index < node.exposedInputs.length}
							<svg
								xmlns="http://www.w3.org/2000/svg"
								viewBox="0 0 8 8"
								class="connector"
								data-connector="input"
								data-tooltip-label={connectorTooltipLabel(secondary)}
								data-tooltip-description={inputTooltipDescription(secondary)}
								data-datatype={secondary.dataType}
								style:--data-color={`var(--color-data-${secondary.dataType.toLowerCase()})`}
								style:--data-color-dim={`var(--color-data-${secondary.dataType.toLowerCase()}-dim)`}
							>
								{#if secondary.connected}
									<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color)" />
								{:else}
									<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color-dim)" />
								{/if}
							</svg>
						{/if}
					{/each}
				</div>
				<!-- Output connectors -->
				<div class="output connectors">
					{#if node.primaryOutput}
						<svg
							xmlns="http://www.w3.org/2000/svg"
							viewBox="0 0 8 8"
							class="connector primary-connector"
							data-connector="output"
							data-tooltip-label={connectorTooltipLabel(node.primaryOutput)}
							data-tooltip-description={outputTooltipDescription(node.primaryOutput)}
							data-datatype={node.primaryOutput.dataType}
							style:--data-color={`var(--color-data-${node.primaryOutput.dataType.toLowerCase()})`}
							style:--data-color-dim={`var(--color-data-${node.primaryOutput.dataType.toLowerCase()}-dim)`}
						>
							{#if node.primaryOutput.connected}
								<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color)" />
							{:else}
								<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color-dim)" />
							{/if}
						</svg>
					{/if}
					{#each node.exposedOutputs as secondary}
						<svg
							xmlns="http://www.w3.org/2000/svg"
							viewBox="0 0 8 8"
							class="connector"
							data-connector="output"
							data-tooltip-label={connectorTooltipLabel(secondary)}
							data-tooltip-description={outputTooltipDescription(secondary)}
							data-datatype={secondary.dataType}
							style:--data-color={`var(--color-data-${secondary.dataType.toLowerCase()})`}
							style:--data-color-dim={`var(--color-data-${secondary.dataType.toLowerCase()}-dim)`}
						>
							{#if secondary.connected}
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

		<!-- Redirect dots, which are Passthrough nodes bending the wires running through them -->
		{#each Array.from($nodeGraph.nodes)
			.filter(([nodeId, node]) => node.isRedirectDot && $visibleNodes.has(nodeId))
			.map(([_, node]) => node) as node (node.id)}
			<div
				class="redirect-dot"
				class:selected={$nodeGraph.selected.includes(node.id)}
				class:previewed={node.previewed}
				style:--offset-left={(node.position?.[0] || 0) + glideCells($nodeGlideOffsets.nodes.get(node.id), 0)}
				style:--offset-top={(node.position?.[1] || 0) + glideCells($nodeGlideOffsets.nodes.get(node.id), 1)}
				style:--data-color={`var(--color-data-${(node.primaryOutput?.dataType || "General").toLowerCase()})`}
				style:--data-color-dim={`var(--color-data-${(node.primaryOutput?.dataType || "General").toLowerCase()}-dim)`}
				data-tooltip-label={node.primaryOutput ? connectorTooltipLabel(node.primaryOutput) : node.displayName}
				data-tooltip-description={node.primaryOutput ? outputTooltipDescription(node.primaryOutput) : ""}
				data-node={node.id}
			>
				<!-- The hover zones mirror the click targets in the backend: the middle and (once wired) left gutter move the dot, and the right gutter pulls out a new wire -->
				<div class="zone move"></div>
				{#if !node.primaryInput?.connected}
					<div class="zone wire-in"></div>
				{/if}
				<div class="zone wire-out"></div>
				<!-- Where both its wires attach, so it stands in for its connectors as the end of a wire being dragged -->
				<div class="dot" class:unwired={!node.primaryInput?.connected && !node.primaryOutput?.connected} data-connector="dot"></div>
				<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8" class="wire-out-port">
					<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color)" />
				</svg>
			</div>
		{/each}
	</div>
</div>

<!-- Box selection widget -->
{#if $nodeGraph.box}
	<div
		class="box-selection"
		style:left={`${Math.min($nodeGraph.box.startX, $nodeGraph.box.endX)}px`}
		style:top={`${Math.min($nodeGraph.box.startY, $nodeGraph.box.endY)}px`}
		style:width={`${Math.abs($nodeGraph.box.startX - $nodeGraph.box.endX)}px`}
		style:height={`${Math.abs($nodeGraph.box.startY - $nodeGraph.box.endY)}px`}
	></div>
{/if}

<style lang="scss">
	.graph {
		position: relative;
		overflow: hidden;
		display: flex;
		flex-direction: row;
		flex-grow: 1;

		// The outline around a hovered connector or redirect dot, set where each one's data type color is
		.connector,
		.redirect-dot {
			--hover-outline-color: var(--data-color-dim);
		}

		.grid-background {
			position: absolute;
			width: 100%;
			height: 100%;
			pointer-events: none;

			// We're displaying the dotted grid in a pseudo-element because `image-rendering` is an inherited property and we don't want it to apply to child elements
			&::before {
				content: "";
				position: absolute;
				width: 100%;
				height: 100%;
				background-size: var(--grid-spacing) var(--grid-spacing);
				background-position: calc(var(--grid-offset-x) - var(--grid-dot-radius)) calc(var(--grid-offset-y) - var(--grid-dot-radius));
				background-image: radial-gradient(circle at var(--grid-dot-radius) var(--grid-dot-radius), rgb(from var(--color-f-white) r g b / 0.2) var(--grid-dot-radius), transparent 0);
				background-repeat: repeat;
				image-rendering: pixelated;
			}
		}

		> img {
			position: absolute;
			bottom: 0;
		}

		.breadcrumb-trail-buttons {
			margin-top: 8px;
			margin-left: 8px;
		}

		// The field for renaming a layer, import or export in place, with padding that grows outward so the text stays where it was
		.name-input {
			color: inherit;
			background: var(--color-1-nearblack);
			border: none;
			outline: none; // Ok for input element
			margin: 0 -4px;
			padding: 0 4px;
			height: 24px;
			border-radius: 2px;
			field-sizing: content;
		}

		.context-menu {
			width: max-content;

			.modify-node-menu {
				margin: -4px;

				.text-button {
					justify-content: left;
				}
			}

			.tail {
				display: none;
			}
		}

		.description-popover {
			.text-area-input {
				width: 300px;

				textarea {
					height: 200px;
				}
			}
		}

		.node-error-container {
			position: absolute;
			z-index: 1;

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
				transition: opacity 0.2s;
				opacity: 0.5;
				transform: translateY(-100%);

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
					user-select: text;
					transition:
						opacity 0.2s,
						z-index 0s 0.2s;

					&::selection {
						background-color: var(--color-e-nearwhite);

						// Target only Safari
						@supports (background: -webkit-named-image(i)) {
							& {
								// Setting an alpha value opts out of Safari's "fancy" (but not visible on dark backgrounds) selection highlight rendering
								// https://stackoverflow.com/a/71753552/775283
								background-color: rgb(from var(--color-e-nearwhite) r g b / calc(254 / 255));
							}
						}
					}
				}
			}
		}

		.click-targets {
			position: absolute;
			pointer-events: none;
			width: 100%;
			height: 100%;
			z-index: 10;

			svg {
				overflow: visible;
				width: 100%;
				height: 100%;
				stroke-width: 1;
				fill: none;

				.layer {
					stroke: yellow;
				}

				.node {
					stroke: blue;
				}

				.connector {
					stroke: green;
				}

				.visibility {
					stroke: red;
				}

				.all-nodes-bounding-box {
					stroke: purple;
				}

				.import-export-name {
					stroke: orange;
				}
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
					stroke-dasharray: var(--data-dasharray);
					transition: stroke 0.1s;
				}

				// The previewed node's temporary wire stands out in the bright data color, as does the wire a dragged node would be dropped onto
				.dashed path,
				.undimmed path {
					stroke: var(--data-color);
				}
			}
		}

		.imports-and-exports {
			width: 100%;
			height: 100%;
			position: absolute;
			pointer-events: none;
			// Keeps the connectors above the wires
			z-index: 1;

			// Zero specificity with `:where()` to allow other rules to override `pointer-events`
			:where(.graph-view.open .graph .imports-and-exports > *) {
				pointer-events: auto;
			}

			.connector {
				position: absolute;
				width: 8px;
				height: 8px;
				margin-top: 4px;
				margin-left: 5px;
				top: calc(var(--offset-top) * 24px);
				left: calc(var(--offset-left) * 24px);
			}

			.reorder-bar {
				position: absolute;
				top: calc(var(--offset-top) * 24px);
				left: calc(var(--offset-left) * 24px);
				width: 50px;
				height: 2px;
				background: white;
			}

			.edit-import-export {
				position: absolute;
				display: flex;
				align-items: center;
				top: calc(var(--offset-top) * 24px);
				margin-top: -5px;
				height: 24px;

				&.separator-bottom::after,
				&.separator-top::before {
					content: "";
					position: absolute;
					background: var(--color-8-uppergray);
					height: 1px;
					left: -4px;
					right: -4px;
				}

				// Centered in the grid cell of space between the primary and the secondary rows
				&.separator-bottom::after {
					bottom: -13px;
				}

				&.separator-top::before {
					top: -12px;
				}

				&.import {
					right: calc(100% - var(--offset-left) * 24px);
				}

				&.export {
					left: calc(var(--offset-left) * 24px + 17px);
				}

				.import-text {
					text-align: right;
					text-wrap: nowrap;
				}

				.export-text {
					text-wrap: nowrap;
				}
			}
		}

		.layers-and-nodes {
			position: absolute;
			pointer-events: none;
			width: 100%;
			height: 100%;

			// Zero specificity with `:where()` to allow other rules to override `pointer-events`
			:where(.graph-view.open .graph .layers-and-nodes > *) {
				pointer-events: auto;
			}
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
			background: rgb(from var(--color-0-black) r g b / 0.33);

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
				background: rgb(from var(--color-4-dimgray) r g b / 0.33);
				color: var(--color-a-softgray);

				.icon-label {
					fill: var(--color-a-softgray);
				}
			}

			&.previewed::after {
				border: 1px dashed var(--color-e-nearwhite);
			}

			.connectors {
				position: absolute;
				// Keeps the connectors above the wires
				z-index: 1;

				&.input {
					left: -3px;
				}

				&.output {
					right: -5px;
				}
			}

			.connector {
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
			--extra-width-to-reach-grid-multiple: 8px;
			--node-chain-area-left-extension: 0;
			// Keep this equation in sync with the equivalent one in the Svelte template `<clipPath><path d="layerBorderMask(...)" /></clipPath>` above, as well as the `left` connector offset CSS rule above in `.connectors.input` above.
			width: calc((var(--layer-area-width) - 0.5) * 24px);
			padding-left: calc(var(--node-chain-area-left-extension) * 24px);
			margin-left: calc((0.5 - var(--node-chain-area-left-extension)) * 24px);

			&::after {
				border: 1px solid var(--color-5-dullgray);
				border-radius: 8px;
			}

			&.selected {
				background: rgb(from var(--color-5-dullgray) r g b / 0.33);

				&.in-selected-network {
					background: rgb(from var(--color-6-lowergray) r g b / 0.33);
				}
			}

			.thumbnail {
				background: var(--color-2-mildblack);
				border: 1px solid var(--color-5-dullgray);
				border-radius: 2px;
				position: relative;
				box-sizing: border-box;
				height: 48px;
				// We shorten the width by 1px on the left and right so the inner thumbnail graphic maintains a perfect 3:2 aspect ratio
				width: calc(72px - 2px);
				margin: 0 1px;

				&::before {
					content: "";
					background-image: var(--color-transparent-checkered-background);
					background-size: var(--color-transparent-checkered-background-size);
					background-position: var(--color-transparent-checkered-background-position);
					background-repeat: var(--color-transparent-checkered-background-repeat);
				}

				&::before,
				svg:not(.connector) {
					pointer-events: none;
					position: absolute;
					margin: auto;
					top: 1px;
					left: 1px;
					width: calc(100% - 2px);
					height: calc(100% - 2px);
				}

				.connector {
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
				display: flex;
				align-items: center;
				margin: 0 8px;

				.text-label {
					white-space: nowrap;
					line-height: 48px;
				}

				.name-input {
					// Stack above the absolutely-positioned grip/lock/visibility siblings, which can otherwise overlap the input's right edge and hijack clicks there.
					position: relative;
					z-index: 2;
				}
			}

			.solo-drag-grip {
				width: 8px;
				height: 24px;
				background-position: 2px 8px;
				right: calc(-12px + 24px);
				border-radius: 2px;
			}

			&.locked .solo-drag-grip {
				right: calc(-12px + 24px + 24px);
			}

			.solo-drag-grip:hover,
			&.selected .solo-drag-grip {
				background-image: var(--icon-drag-grip);

				&:hover {
					background-image: var(--icon-drag-grip-hover);
				}
			}

			.visibility {
				right: -12px;
			}

			.lock {
				right: 12px;
			}

			.input.connectors {
				left: calc(-3px + var(--node-chain-area-left-extension) * 24px - 36px);
			}

			.solo-drag-grip,
			.lock,
			.visibility,
			.input.connectors,
			.input.connectors .connector {
				position: absolute;
				margin: auto 0;
				top: 0;
				bottom: 0;
			}

			// Above layers drawn later whose chain area extends over this layer's right edge, so hovering still reaches them
			.solo-drag-grip,
			.lock,
			.visibility {
				z-index: 1;
			}

			.input.connectors .connector {
				left: 24px;
			}
		}

		.redirect-dot {
			position: absolute;
			left: calc(var(--offset-left) * 24px - 15px);
			top: calc((var(--offset-top) + 1) * 24px - 8px);
			// Matches the dot's click target in the backend, which also covers the port gutters to either side
			width: 30px;
			height: 16px;
			display: flex;
			align-items: center;
			justify-content: center;

			.zone {
				position: absolute;
				z-index: 1;

				// Left and middle, where pressing selects the dot and dragging moves it
				&.move {
					inset: 0;
				}

				// Right gutter, matching the output port's click target (a 10px circle centered 10px right of the dot)
				&.wire-out {
					left: 20px;
					top: 3px;
					width: 10px;
					height: 10px;
					border-radius: 50%;
					z-index: 2;
				}

				// Left gutter of a dot with nothing feeding it, matching the input port's click target there, which pulls a wire out rather than moving the dot
				&.wire-in {
					left: 0;
					top: 3px;
					width: 10px;
					height: 10px;
					border-radius: 50%;
					z-index: 2;
				}
			}

			.dot {
				box-sizing: border-box;
				width: 10px;
				height: 10px;
				border-radius: 50%;
				background: var(--data-color);
				// Faded in on hover
				outline: 2px solid transparent;
				pointer-events: none;
				transition:
					transform 0.1s,
					background-color 0.1s,
					outline-color 0.1s;

				&.unwired {
					background: var(--data-color-dim);
				}
			}

			// The same connector arrow nodes have on their outputs, offered where a new wire can be pulled out
			.wire-out-port {
				position: absolute;
				left: 21px;
				top: 4px;
				width: 8px;
				height: 8px;
				overflow: visible;
				pointer-events: none;
				opacity: 0;
				transition: opacity 0.1s;
			}

			&.selected .dot {
				outline: 2px solid rgb(from var(--color-f-white) r g b / 0.5);
			}

			&.previewed .dot {
				outline: 1px dashed var(--data-color);
				outline-offset: 2px;
			}
		}

		// Hover hints stay hidden while a pointer button is held, since a drag may be underway, except on the dot being pressed.
		// Hovering to select or drag enlarges the dot and rings it.
		&:not(.pointer-held) .redirect-dot .zone.move:hover ~ .dot,
		.redirect-dot .zone.move:active ~ .dot {
			transform: scale(1.3);
			outline: 2px solid var(--hover-outline-color);
		}

		// Hovering the right gutter instead dims the dot and reveals the connector to pull a wire from
		&:not(.pointer-held) .redirect-dot .zone.wire-out:hover ~ .dot {
			background-color: var(--data-color-dim);
		}

		&:not(.pointer-held) .redirect-dot .zone.wire-out:hover ~ .wire-out-port {
			opacity: 1;
		}

		// A wire dragged out of the dot keeps showing its connector as hovering the gutter did, and one snapped onto it rings it like a hover
		.redirect-dot .dot.wire-source {
			background-color: var(--data-color-dim);

			~ .wire-out-port {
				opacity: 1;
			}
		}

		.redirect-dot .dot.wire-target {
			transform: scale(1.3);
			outline: 2px solid var(--hover-outline-color);
		}

		// Hovering a connector enlarges it and outlines its arrow, like the ring a hovered redirect dot gets
		.connector,
		.redirect-dot .wire-out-port {
			overflow: visible;

			// Enlarged about the arrow's own center, leaving the ends of a thick stack wire drawn within a layer's connector as they are
			path {
				transform-box: fill-box;
				transform-origin: center;
				transition:
					fill 0.1s,
					transform 0.1s;
			}
		}

		&:not(.pointer-held) .connector.hovered,
		.connector.wire-target,
		&:not(.pointer-held) .redirect-dot .zone.wire-out:hover ~ .wire-out-port {
			// Undimmed if disconnected, with the stroke painted beneath the fill, so only its outer half shows
			path:not(.stack-wire-end) {
				transform: scale(1.3);
				fill: var(--data-color);
				stroke: var(--hover-outline-color);
				stroke-width: 4px;
				stroke-linejoin: round;
				paint-order: stroke;
			}
		}

		// The connector a wire is dragged from stays undimmed, as it was while hovered
		.connector.wire-source path:not(.stack-wire-end) {
			fill: var(--data-color);
		}

		.node {
			flex-direction: column;
			border-radius: 2px;
			width: 120px;
			top: calc((var(--offset-top) + 0.5) * 24px);

			&::after {
				border: 1px solid var(--color-5-dullgray);
				border-radius: 2px;
			}

			&.selected {
				.primary {
					background: rgb(from var(--color-f-white) r g b / 0.15);

					&.in-selected-network {
						background: rgb(from var(--color-f-white) r g b / 0.2);
					}
				}

				.secondary {
					background: rgb(from var(--color-f-white) r g b / 0.1);

					&.in-selected-network {
						background: rgb(from var(--color-f-white) r g b / 0.15);
					}
				}
			}

			.connector {
				&:first-of-type {
					margin-top: calc((24px - 8px) / 2);

					&:not(.primary-connector) {
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
				background: rgb(from var(--color-f-white) r g b / 0.05);

				&.no-secondary-section {
					border-radius: 2px;
				}

				.icon-label {
					margin: 0 8px;
				}

				.text-label {
					margin: 0 8px;
				}

				.icon-label + .text-label {
					margin-left: 0;
					margin-right: 4px;
				}
			}

			.secondary {
				display: flex;
				flex-direction: column;
				width: 100%;
				position: relative;

				.secondary-row {
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
		pointer-events: none;
		z-index: 2;
		// TODO: This will be removed after box selection, and all of graph rendering, is moved to the backend and this whole file
		// is removed, but for now this color needs to stay in sync with `COLOR_OVERLAY_BLUE` set in consts.rs of the editor backend.
		background: rgba(0, 168, 255, 0.05);
		border: 1px solid #00a8ff;
	}
</style>

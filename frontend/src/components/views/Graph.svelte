<script lang="ts">
	import { getContext, onDestroy, onMount, tick } from "svelte";
	import NodeCatalog from "/src/components/floating-menus/NodeCatalog.svelte";
	import FloatingMenu from "/src/components/layout/FloatingMenu.svelte";
	import LayoutCol from "/src/components/layout/LayoutCol.svelte";
	import TextButton from "/src/components/widgets/buttons/TextButton.svelte";
	import TextAreaInput from "/src/components/widgets/inputs/TextAreaInput.svelte";
	import IconLabel from "/src/components/widgets/labels/IconLabel.svelte";
	import TextLabel from "/src/components/widgets/labels/TextLabel.svelte";
	import type { DocumentStore } from "/src/stores/document";
	import type { NodeGraphStore } from "/src/stores/node-graph";
	import { closeContextMenu } from "/src/stores/node-graph";
	import { retargetTooltip } from "/src/stores/tooltip";
	import type { SubscriptionsRouter } from "/src/subscriptions-router";
	import { onFloatingMenuOpenChange } from "/src/utility-functions/input";
	import { operatingSystem } from "/src/utility-functions/platform";
	import type { EditorWrapper, GraphNameField } from "/wrapper/pkg/graphite_wasm_wrapper";

	const editor = getContext<EditorWrapper>("editor");
	const nodeGraph = getContext<NodeGraphStore>("nodeGraph");
	const nodeGraphTransform = nodeGraph.transformStore;
	const documentState = getContext<DocumentStore>("document");
	const subscriptions = getContext<SubscriptionsRouter>("subscriptions");

	// Close the context menu when the graph view overlay is closed
	$: if (!$documentState.graphViewOverlayOpen) closeContextMenu();

	let graph: HTMLDivElement | undefined;

	// The tooltip of the part of the engine-drawn graph under the pointer, which the editor finds
	let tooltip = { label: "", description: "" };

	// The field a name of the engine-drawn graph is edited in, laid over the name the editor leaves undrawn meanwhile
	let nameField: GraphNameField | undefined = undefined;
	let nameFieldText = "";
	let nameFieldInput: HTMLInputElement | undefined;

	// Once only, since removing the field can blur it after Enter, Escape or the editor has already ended the edit
	function endNameEdit(name: string | undefined) {
		if (!nameField) return;

		nameField = undefined;
		editor.endGraphNameEdit(name);
		onFloatingMenuOpenChange("graph-name-field", false, editor);
	}

	function onNameFieldKeyDown(e: KeyboardEvent) {
		if (e.key === "Enter") endNameEdit(nameFieldText);
		else if (e.key === "Escape") endNameEdit(undefined);
		else return;

		e.stopPropagation();
	}

	// The import or export with its description open for editing in a popover pointing at `anchor`, in graph space
	let editingDescription: { isImport: boolean; index: number; anchor: [number, number]; text: string; committedText: string } | undefined = undefined;
	let descriptionInput: TextAreaInput | undefined;

	async function editDescription(isImport: boolean, index: number, description: string, anchor: [number, number]) {
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

	function closeDescriptionPopover() {
		commitDescription();
		editingDescription = undefined;
	}

	// Ctrl/Cmd+Enter commits the description like in any text area, and here it also closes the popover
	function onDescriptionKeyDown(e: KeyboardEvent) {
		const commandKey = operatingSystem() === "Mac" ? e.metaKey : e.ctrlKey;
		if (commandKey && e.key === "Enter") closeDescriptionPopover();
	}

	onMount(() => {
		subscriptions.subscribeFrontendMessage("UpdateGraphNameField", async (data) => {
			nameField = data.field;
			// Counted as an open floating menu, so on desktop the presses in the field stay in it rather than going straight to the editor
			onFloatingMenuOpenChange("graph-name-field", nameField !== undefined, editor);
			if (!data.field || !data.opens) return;

			nameFieldText = data.field.name;
			await tick();
			nameFieldInput?.focus();
			nameFieldInput?.select();
		});

		subscriptions.subscribeFrontendMessage("UpdateGraphTooltip", (data) => {
			tooltip = data;

			// Left alone over a menu or field within the graph, which has its own tooltips
			if (graph?.matches(":hover") && !graph.querySelector(":hover")) retargetTooltip(data.label || data.description ? graph : undefined);
		});
	});

	onDestroy(() => {
		subscriptions.unsubscribeFrontendMessage("UpdateGraphNameField");
		subscriptions.unsubscribeFrontendMessage("UpdateGraphTooltip");
		onFloatingMenuOpenChange("graph-name-field", false, editor);
	});

	function createNode(identifier: string) {
		if ($nodeGraph.contextMenuInformation === undefined) return;

		editor.createNode(identifier, $nodeGraph.contextMenuInformation.nodeCreationCoordinates[0], $nodeGraph.contextMenuInformation.nodeCreationCoordinates[1]);
	}
</script>

<div class="graph" bind:this={graph} data-node-graph data-tooltip-label={tooltip.label || undefined} data-tooltip-description={tooltip.description || undefined}>
	<!-- Kept out of the graph's pointer input so its button doesn't also start a box selection -->
	{#if $nodeGraph.readOnly}
		<div class="read-only-banner" on:pointerdown|stopPropagation on:wheel|passive|stopPropagation>
			<IconLabel icon="PadlockLocked" />
			<TextLabel>The contents of this built-in node are shared by all its instances and read-only</TextLabel>
			<TextButton label="Edit Contents" action={() => editor.makeNetworkEditable()} />
		</div>
	{/if}
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
					{#if $nodeGraph.contextMenuInformation.contextMenuData.data.isShared}
						{@const nodeId = $nodeGraph.contextMenuInformation.contextMenuData.data.nodeId}
						<TextButton
							label="Make Unique"
							action={() => {
								editor.makeNodeUnique(nodeId);
								closeContextMenu();
							}}
							flush={true}
						/>
					{/if}
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
				{@const description = $nodeGraph.contextMenuInformation.contextMenuData.data.description}
				{@const descriptionAnchor = $nodeGraph.contextMenuInformation.contextMenuData.data.descriptionAnchor}
				<LayoutCol class="modify-node-menu">
					<TextButton
						label="Rename"
						action={() => {
							editor.beginImportExportNameEdit(isImport, index);
							closeContextMenu();
						}}
						flush={true}
					/>
					<TextButton
						label="Edit Description"
						action={() => {
							editDescription(isImport, index, description, descriptionAnchor);
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

	{#if nameField}
		<div class="name-field" style:transform-origin="0 0" style:transform={`translate(${$nodeGraphTransform.x}px, ${$nodeGraphTransform.y}px) scale(${$nodeGraphTransform.scale})`}>
			<input
				class="name-input"
				class:dimmed={nameField.dimmed}
				style:left={nameField.alignRight ? undefined : `${nameField.x}px`}
				style:right={nameField.alignRight ? `${-nameField.x}px` : undefined}
				style:top={`${nameField.top}px`}
				type="text"
				bind:this={nameFieldInput}
				bind:value={nameFieldText}
				on:pointerdown|stopPropagation
				on:blur={() => endNameEdit(nameFieldText)}
				on:keydown={onNameFieldKeyDown}
			/>
		</div>
	{/if}
</div>

<style lang="scss">
	.graph {
		position: relative;
		overflow: hidden;
		display: flex;
		flex-direction: row;
		flex-grow: 1;

		.read-only-banner {
			position: absolute;
			top: 8px;
			left: 50%;
			transform: translateX(-50%);
			z-index: 1;
			display: flex;
			align-items: center;
			gap: 8px;
			padding: 4px 4px 4px 8px;
			border-radius: 4px;
			background: var(--color-3-darkgray);
			white-space: nowrap;
		}

		// The field for renaming a layer, import or export in place, with padding that grows outward so the text stays where it was
		.name-input {
			position: absolute;
			color: inherit;
			background: var(--color-1-nearblack);
			border: none;
			outline: none; // Ok for input element
			margin: 0 -4px;
			padding: 0 4px;
			height: 24px;
			border-radius: 2px;
			field-sizing: content;

			&.dimmed {
				color: var(--color-a-softgray);
			}
		}

		.name-field {
			position: absolute;
			top: 0;
			left: 0;
			z-index: 1;
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
	}
</style>

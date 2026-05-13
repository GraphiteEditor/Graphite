<script lang="ts">
	import { getContext, onDestroy } from "svelte";
	import LayoutCol from "/src/components/layout/LayoutCol.svelte";
	import LayoutRow from "/src/components/layout/LayoutRow.svelte";
	import Panel from "/src/components/window/Panel.svelte";
	import type { PortfolioStore } from "/src/stores/portfolio";
	import type { DocumentInfo, EditorWrapper, PanelGroupState, PanelLayoutSubdivision } from "/wrapper/pkg/graphite_wasm_wrapper";

	const MIN_PANEL_SIZE = 100;
	const DOUBLE_CLICK_MILLISECONDS = 500;
	// Must match DOCUMENT_PANEL_SHARE / NON_DOCUMENT_PANEL_SHARE in utility_types.rs
	const DOCUMENT_PANEL_SHARE = 0.8;
	const EQUAL_PANEL_SHARE = 0.5;

	const editor = getContext<EditorWrapper>("editor");
	const portfolio = getContext<PortfolioStore>("portfolio");

	export let subdivision: PanelLayoutSubdivision | undefined;
	export let depth: number;
	export let splitPath: number[] = [];

	// Local size overrides for gutter resizing (keyed by child index)
	let sizeOverrides: Record<number, number> = {};
	// Gutter resize state
	let gutterResizeRestore: [number, number] | undefined = undefined;
	let pointerCaptureId: number | undefined = undefined;
	let activeResizeCleanup: (() => void) | undefined = undefined;
	let lastGutterClickTarget: EventTarget | undefined = undefined;
	let lastGutterClickTime = 0;
	let lastSubdivisionRef: PanelLayoutSubdivision | undefined = undefined;

	// At even depths (0, 2, 4...) children are in a row, at odd depths (1, 3, 5...) in a column
	$: horizontal = depth % 2 === 0;
	// Compare by reference because `safe_not_equal` treats any store update as changed, which would wipe drag overrides
	$: if (subdivision !== lastSubdivisionRef) {
		sizeOverrides = {};
		lastSubdivisionRef = subdivision;
	}
	// Reactive array of resolved sizes (merging backend defaults with local overrides)
	$: resolvedSizes = subdivision && "Split" in subdivision ? subdivision.Split.children.map((child, index) => sizeOverrides[index] ?? child.size) : [];
	$: documentTabLabels = $portfolio.documents.map((doc: DocumentInfo) => {
		const name = doc.name;
		const unsaved = !doc.is_saved;
		if (!editor.inDevelopmentMode()) return { name, unsaved };

		const tooltipDescription = `Document ID: ${doc.id}`;
		return { name, unsaved, tooltipLabel: name, tooltipDescription };
	});

	onDestroy(() => {
		activeResizeCleanup?.();
	});

	function resizePanel(e: PointerEvent, prevIndex: number, nextIndex: number) {
		if (!(subdivision && "Split" in subdivision)) return;

		const gutter = e.target;
		if (!(gutter instanceof HTMLDivElement)) return;

		const nextSibling = gutter.nextElementSibling;
		const prevSibling = gutter.previousElementSibling;
		const parentElement = gutter.parentElement;
		if (!(nextSibling instanceof HTMLDivElement) || !(prevSibling instanceof HTMLDivElement) || !(parentElement instanceof HTMLDivElement)) return;

		// Double-click resets the two adjacent panels to the default ratio (80:20 near document, otherwise 50:50)
		const now = Date.now();
		const isDoubleClick = now - lastGutterClickTime < DOUBLE_CLICK_MILLISECONDS && lastGutterClickTarget === gutter;

		lastGutterClickTime = now;
		lastGutterClickTarget = gutter;

		if (isDoubleClick) {
			const children = subdivision.Split.children;
			const adjacentSum = resolvedSizes[prevIndex] + resolvedSizes[nextIndex];

			const prevHasDocument = subtreeContainsDocument(children[prevIndex].subdivision);
			const nextHasDocument = subtreeContainsDocument(children[nextIndex].subdivision);

			let prevShare = EQUAL_PANEL_SHARE;
			if (prevHasDocument && !nextHasDocument) prevShare = DOCUMENT_PANEL_SHARE;
			else if (!prevHasDocument && nextHasDocument) prevShare = 1 - DOCUMENT_PANEL_SHARE;

			sizeOverrides[prevIndex] = adjacentSum * prevShare;
			sizeOverrides[nextIndex] = adjacentSum * (1 - prevShare);
			sizeOverrides = sizeOverrides;

			const allSizes = children.map((child, i) => sizeOverrides[i] ?? child.size);
			editor.setPanelGroupSizes(splitPath, allSizes);
			return;
		}

		const isHorizontal = horizontal;

		const nextSiblingSize = isHorizontal ? nextSibling.getBoundingClientRect().width : nextSibling.getBoundingClientRect().height;
		const prevSiblingSize = isHorizontal ? prevSibling.getBoundingClientRect().width : prevSibling.getBoundingClientRect().height;

		// Only redistribute within the two adjacent panels' combined flex-grow total
		const adjacentFlexGrowTotal = resolvedSizes[prevIndex] + resolvedSizes[nextIndex];
		const adjacentPixelTotal = prevSiblingSize + nextSiblingSize;

		pointerCaptureId = e.pointerId;
		gutter.setPointerCapture(pointerCaptureId);

		const mouseStart = isHorizontal ? e.clientX : e.clientY;

		const abortResize = () => {
			if (pointerCaptureId) gutter.releasePointerCapture(pointerCaptureId);
			pointerCaptureId = undefined;
			removeListeners();
			activeResizeCleanup = undefined;

			if (gutterResizeRestore !== undefined) {
				sizeOverrides[nextIndex] = gutterResizeRestore[0];
				sizeOverrides[prevIndex] = gutterResizeRestore[1];
				sizeOverrides = sizeOverrides;
				gutterResizeRestore = undefined;
			}
		};

		const onPointerMove = (e: PointerEvent) => {
			const mouseCurrent = isHorizontal ? e.clientX : e.clientY;
			let mouseDelta = mouseStart - mouseCurrent;

			mouseDelta = Math.max(nextSiblingSize + mouseDelta, MIN_PANEL_SIZE) - nextSiblingSize;
			mouseDelta = prevSiblingSize - Math.max(prevSiblingSize - mouseDelta, MIN_PANEL_SIZE);

			if (gutterResizeRestore === undefined) gutterResizeRestore = [resolvedSizes[nextIndex], resolvedSizes[prevIndex]];

			sizeOverrides[nextIndex] = (adjacentFlexGrowTotal * (nextSiblingSize + mouseDelta)) / adjacentPixelTotal;
			sizeOverrides[prevIndex] = (adjacentFlexGrowTotal * (prevSiblingSize - mouseDelta)) / adjacentPixelTotal;
			sizeOverrides = sizeOverrides;
		};

		const onPointerUp = () => {
			gutterResizeRestore = undefined;
			if (pointerCaptureId) gutter.releasePointerCapture(pointerCaptureId);
			removeListeners();
			activeResizeCleanup = undefined;

			// Persist the resized sizes to the backend
			if ("Split" in subdivision) {
				const allSizes = subdivision.Split.children.map((child, i) => sizeOverrides[i] ?? child.size);
				editor.setPanelGroupSizes(splitPath, allSizes);
			}
		};

		const onMouseDown = (e: MouseEvent) => {
			const BUTTONS_RIGHT = 0b0000_0010;
			if (e.buttons & BUTTONS_RIGHT) abortResize();
		};

		const onKeyDown = (e: KeyboardEvent) => {
			if (e.key === "Escape") abortResize();
		};

		const addListeners = () => {
			document.addEventListener("pointermove", onPointerMove);
			document.addEventListener("pointerup", onPointerUp);
			document.addEventListener("mousedown", onMouseDown);
			document.addEventListener("keydown", onKeyDown);
		};

		const removeListeners = () => {
			document.removeEventListener("pointermove", onPointerMove);
			document.removeEventListener("pointerup", onPointerUp);
			document.removeEventListener("mousedown", onMouseDown);
			document.removeEventListener("keydown", onKeyDown);
		};

		addListeners();
		activeResizeCleanup = removeListeners;
	}

	function crossPanelDrop(sourcePanelId: string, targetPanelId: string, insertIndex: number) {
		editor.movePanelTab(BigInt(sourcePanelId), BigInt(targetPanelId), insertIndex);
	}

	function groupDrop(sourcePanelId: string, targetPanelId: string, insertIndex: number) {
		editor.moveAllPanelTabs(BigInt(sourcePanelId), BigInt(targetPanelId), insertIndex);
	}

	function splitDrop(targetPanelId: string, direction: string, tabs: string[], activeTabIndex: number) {
		editor.splitPanelGroup(BigInt(targetPanelId), direction, tabs, activeTabIndex);
	}

	function isDocumentGroup(state: PanelGroupState): boolean {
		return state.tabs.some((t) => t === "Document" || t === "Welcome");
	}

	function subtreeContainsDocument(node: PanelLayoutSubdivision): boolean {
		if ("PanelGroup" in node) return isDocumentGroup(node.PanelGroup.state);
		if ("Split" in node) return node.Split.children.some((child) => subtreeContainsDocument(child.subdivision));
		return false;
	}
</script>

{#if subdivision && "PanelGroup" in subdivision}
	{@const group = subdivision.PanelGroup}
	{#if isDocumentGroup(group.state)}
		<Panel
			class="document-panel"
			panelId={String(group.id)}
			panelTypes={$portfolio.documents.length > 0 ? $portfolio.documents.map(() => "Document") : ["Welcome"]}
			tabCloseButtons={true}
			tabMinWidths={true}
			tabLabels={documentTabLabels}
			emptySpaceAction={() => editor.newDocumentDialog()}
			clickAction={(tabIndex) => editor.selectDocument($portfolio.documents[tabIndex].id)}
			closeAction={(tabIndex) => editor.closeDocumentWithConfirmation($portfolio.documents[tabIndex].id)}
			reorderAction={(oldIndex, newIndex) => editor.reorderDocument($portfolio.documents[oldIndex].id, newIndex)}
			renameAction={(tabIndex, newName) => {
				// Ensure the target document is the active one before renaming, since `RenameDocument` operates on the active document
				editor.selectDocument($portfolio.documents[tabIndex].id);
				editor.renameDocument(newName);
			}}
			tabActiveIndex={$portfolio.activeDocumentIndex}
			groupDropAction={groupDrop}
			splitDropAction={splitDrop}
		/>
	{:else}
		<Panel
			panelId={String(group.id)}
			panelTypes={group.state.tabs}
			tabLabels={group.state.tabs.map((name) => ({ name }))}
			tabActiveIndex={Number(group.state.active_tab_index)}
			clickAction={(tabIndex) => editor.setPanelGroupActiveTab(group.id, tabIndex)}
			reorderAction={(oldIndex, newIndex) => editor.reorderPanelGroupTab(group.id, oldIndex, newIndex)}
			crossPanelDropAction={crossPanelDrop}
			groupDropAction={groupDrop}
			splitDropAction={splitDrop}
		/>
	{/if}
{:else if subdivision && "Split" in subdivision}
	{#each subdivision.Split.children as child, index}
		{#if index > 0}
			{#if horizontal}
				<LayoutCol class="workspace-grid-resize-gutter" data-gutter-horizontal on:pointerdown={(e) => resizePanel(e, index - 1, index)} />
			{:else}
				<LayoutRow class="workspace-grid-resize-gutter" data-gutter-vertical on:pointerdown={(e) => resizePanel(e, index - 1, index)} />
			{/if}
		{/if}
		{#if horizontal}
			<LayoutCol class="workspace-grid-subdivision" styles={{ "flex-grow": resolvedSizes[index] }}>
				<svelte:self subdivision={child.subdivision} depth={depth + 1} splitPath={[...splitPath, index]} />
			</LayoutCol>
		{:else}
			<LayoutRow class="workspace-grid-subdivision" styles={{ "flex-grow": resolvedSizes[index] }}>
				<svelte:self subdivision={child.subdivision} depth={depth + 1} splitPath={[...splitPath, index]} />
			</LayoutRow>
		{/if}
	{/each}
{/if}

<style lang="scss">
	.workspace-grid-resize-gutter {
		flex: 0 0 4px;
		border-radius: 2px;
		transition: background 0.2s 0s;

		&.layout-row {
			cursor: ns-resize;
		}

		&.layout-col {
			cursor: ew-resize;
		}

		&:hover {
			background: var(--color-5-dullgray);
			transition: background 0.2s 0.1s;
		}
	}

	.workspace-grid-subdivision {
		position: relative;
		flex: 1 1 0;
		min-height: 28px;

		&.folded {
			flex-grow: 0;
			height: 0;
		}
	}

	// Needed for the viewport hole punch on desktop
	.viewport-hole-punch .workspace-grid-subdivision:has(> .panel.document-panel)::after {
		content: "";
		position: absolute;
		z-index: -1;
		inset: 6px;
		border-radius: 6px;
		box-shadow: 0 0 0 calc(100vw + 100vh) var(--color-2-mildblack);
	}
</style>

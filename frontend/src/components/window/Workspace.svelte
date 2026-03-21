<script lang="ts">
	import { getContext, onDestroy } from "svelte";
	import LayoutCol from "/src/components/layout/LayoutCol.svelte";
	import LayoutRow from "/src/components/layout/LayoutRow.svelte";
	import Panel from "/src/components/window/Panel.svelte";
	import type { PortfolioStore } from "/src/stores/portfolio";
	import type { EditorWrapper, OpenDocument } from "/wrapper/pkg/graphite_wasm_wrapper";

	const MIN_PANEL_SIZE = 100;
	const PANEL_SIZES = {
		/**/ root: 100,
		/*   ├─ */ content: 80,
		/*   │     ├─ */ document: 70,
		/*   │     └─ */ data: 30,
		/*   └─ */ details: 20,
		/*         ├─ */ properties: 45,
		/*         └─ */ layers: 55,
	} as const;

	let panelSizes: Record<string, number> = PANEL_SIZES;
	let documentPanel: Panel | undefined;
	let gutterResizeRestore: [number, number] | undefined = undefined;
	let pointerCaptureId: number | undefined = undefined;
	let activeResizeCleanup: (() => void) | undefined = undefined;

	onDestroy(() => {
		activeResizeCleanup?.();
	});

	$: documentPanel?.scrollTabIntoView($portfolio.activeDocumentIndex);

	$: documentTabLabels = $portfolio.documents.map((doc: OpenDocument) => {
		const name = doc.details.name;
		const unsaved = !doc.details.isSaved;
		if (!editor.inDevelopmentMode()) return { name, unsaved };

		const tooltipDescription = `Document ID: ${doc.id}`;
		return { name, unsaved, tooltipLabel: name, tooltipDescription };
	});

	const editor = getContext<EditorWrapper>("editor");
	const portfolio = getContext<PortfolioStore>("portfolio");

	function resizePanel(e: PointerEvent) {
		const gutter = e.target;
		if (!(gutter instanceof HTMLDivElement)) return;

		const nextSibling = gutter.nextElementSibling;
		const prevSibling = gutter.previousElementSibling;

		const parentElement = gutter.parentElement;
		if (!(nextSibling instanceof HTMLDivElement) || !(prevSibling instanceof HTMLDivElement) || !(parentElement instanceof HTMLDivElement)) return;

		const nextSiblingName = nextSibling.getAttribute("data-subdivision-name") || undefined;
		const prevSiblingName = prevSibling.getAttribute("data-subdivision-name") || undefined;

		if (!nextSiblingName || !prevSiblingName || !(nextSiblingName in PANEL_SIZES) || !(prevSiblingName in PANEL_SIZES)) return;

		// Are we resizing horizontally?
		const isHorizontal = gutter.getAttribute("data-gutter-horizontal") !== null;

		// Get the current size in px of the panels being resized and the gutter
		const gutterSize = isHorizontal ? gutter.getBoundingClientRect().width : gutter.getBoundingClientRect().height;
		const nextSiblingSize = isHorizontal ? nextSibling.getBoundingClientRect().width : nextSibling.getBoundingClientRect().height;
		const prevSiblingSize = isHorizontal ? prevSibling.getBoundingClientRect().width : prevSibling.getBoundingClientRect().height;
		const parentElementSize = isHorizontal ? parentElement.getBoundingClientRect().width : parentElement.getBoundingClientRect().height;

		// Measure the resizing panels as a percentage of all sibling panels
		const totalResizingSpaceOccupied = gutterSize + nextSiblingSize + prevSiblingSize;
		const proportionBeingResized = totalResizingSpaceOccupied / parentElementSize;

		// Prevent cursor flicker as mouse temporarily leaves the gutter
		pointerCaptureId = e.pointerId;
		gutter.setPointerCapture(pointerCaptureId);

		const mouseStart = isHorizontal ? e.clientX : e.clientY;

		const abortResize = () => {
			if (pointerCaptureId) gutter.releasePointerCapture(pointerCaptureId);
			removeListeners();
			activeResizeCleanup = undefined;

			pointerCaptureId = e.pointerId;
			gutter.setPointerCapture(pointerCaptureId);

			if (gutterResizeRestore !== undefined) {
				panelSizes[nextSiblingName] = gutterResizeRestore[0];
				panelSizes[prevSiblingName] = gutterResizeRestore[1];
				gutterResizeRestore = undefined;
			}
		};

		const onPointerMove = (e: PointerEvent) => {
			const mouseCurrent = isHorizontal ? e.clientX : e.clientY;
			let mouseDelta = mouseStart - mouseCurrent;

			mouseDelta = Math.max(nextSiblingSize + mouseDelta, MIN_PANEL_SIZE) - nextSiblingSize;
			mouseDelta = prevSiblingSize - Math.max(prevSiblingSize - mouseDelta, MIN_PANEL_SIZE);

			if (gutterResizeRestore === undefined) gutterResizeRestore = [panelSizes[nextSiblingName], panelSizes[prevSiblingName]];

			panelSizes[nextSiblingName] = ((nextSiblingSize + mouseDelta) / totalResizingSpaceOccupied) * proportionBeingResized * 100;
			panelSizes[prevSiblingName] = ((prevSiblingSize - mouseDelta) / totalResizingSpaceOccupied) * proportionBeingResized * 100;
		};

		const onPointerUp = () => {
			gutterResizeRestore = undefined;
			if (pointerCaptureId) gutter.releasePointerCapture(pointerCaptureId);
			removeListeners();
			activeResizeCleanup = undefined;
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
</script>

<LayoutRow class="workspace" data-workspace>
	<LayoutRow class="workspace-grid-subdivision" styles={{ "flex-grow": panelSizes["root"] }} data-subdivision-name="root">
		<LayoutCol class="workspace-grid-subdivision" styles={{ "flex-grow": panelSizes["content"] }} data-subdivision-name="content">
			<LayoutRow class="workspace-grid-subdivision" styles={{ "flex-grow": panelSizes["document"] }} data-subdivision-name="document">
				<Panel
					class="document-panel"
					panelType={$portfolio.documents.length > 0 ? "Document" : "Welcome"}
					tabCloseButtons={true}
					tabMinWidths={true}
					tabLabels={documentTabLabels}
					emptySpaceAction={() => editor.newDocumentDialog()}
					clickAction={(tabIndex) => editor.selectDocument($portfolio.documents[tabIndex].id)}
					closeAction={(tabIndex) => editor.closeDocumentWithConfirmation($portfolio.documents[tabIndex].id)}
					tabActiveIndex={$portfolio.activeDocumentIndex}
					bind:this={documentPanel}
				/>
			</LayoutRow>
			{#if $portfolio.dataPanelOpen}
				<LayoutRow class="workspace-grid-resize-gutter" data-gutter-vertical on:pointerdown={(e) => resizePanel(e)} />
				<LayoutRow class="workspace-grid-subdivision" styles={{ "flex-grow": panelSizes["data"] }} data-subdivision-name="data">
					<Panel panelType="Data" tabLabels={[{ name: "Data" }]} tabActiveIndex={0} />
				</LayoutRow>
			{/if}
		</LayoutCol>
		{#if $portfolio.propertiesPanelOpen || $portfolio.layersPanelOpen}
			<LayoutCol class="workspace-grid-resize-gutter" data-gutter-horizontal on:pointerdown={(e) => resizePanel(e)} />
			<LayoutCol class="workspace-grid-subdivision" styles={{ "flex-grow": panelSizes["details"] }} data-subdivision-name="details">
				{#if $portfolio.propertiesPanelOpen}
					<LayoutRow class="workspace-grid-subdivision" styles={{ "flex-grow": panelSizes["properties"] }} data-subdivision-name="properties">
						<Panel panelType="Properties" tabLabels={[{ name: "Properties" }]} tabActiveIndex={0} />
					</LayoutRow>
				{/if}
				{#if $portfolio.propertiesPanelOpen && $portfolio.layersPanelOpen}
					<LayoutRow class="workspace-grid-resize-gutter" data-gutter-vertical on:pointerdown={(e) => resizePanel(e)} />
				{/if}
				{#if $portfolio.layersPanelOpen}
					<LayoutRow class="workspace-grid-subdivision" styles={{ "flex-grow": panelSizes["layers"] }} data-subdivision-name="layers">
						<Panel panelType="Layers" tabLabels={[{ name: "Layers" }]} tabActiveIndex={0} />
					</LayoutRow>
				{/if}
			</LayoutCol>
		{/if}
	</LayoutRow>
</LayoutRow>

<style lang="scss" global>
	.workspace {
		position: relative;
		flex: 1 1 100%;

		.workspace-grid-subdivision {
			position: relative;
			flex: 1 1 0;
			min-height: 28px;

			&.folded {
				flex-grow: 0;
				height: 0;
			}
		}

		.workspace-grid-resize-gutter {
			flex: 0 0 4px;

			&.layout-row {
				cursor: ns-resize;
			}

			&.layout-col {
				cursor: ew-resize;
			}
		}

		// Needed for the viewport hole punch on desktop
		.viewport-hole-punch & .workspace-grid-subdivision:has(.panel.document-panel)::after {
			content: "";
			position: absolute;
			inset: 6px;
			border-radius: 6px;
			box-shadow: 0 0 0 calc(100vw + 100vh) var(--color-2-mildblack);
			z-index: -1;
		}
	}
</style>

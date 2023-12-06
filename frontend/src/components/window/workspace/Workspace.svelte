<script lang="ts">
	import { getContext } from "svelte";

	import type { DialogState } from "@graphite/state-providers/dialog";
	import type { PortfolioState } from "@graphite/state-providers/portfolio";
	import type { Editor } from "@graphite/wasm-communication/editor";

	import type { FrontendDocumentDetails } from "@graphite/wasm-communication/messages";

	import Dialog from "@graphite/components/floating-menus/Dialog.svelte";
	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import Panel from "@graphite/components/window/workspace/Panel.svelte";

	const MIN_PANEL_SIZE = 100;
	const PANEL_SIZES = {
		/**/ root: 100,
		/*   ├── */ content: 80,
		/*   │      ├── */ document: 100,
		/*   └── */ details: 20,
		/*          ├── */ properties: 45,
		/*          └── */ layers: 55,
	};

	let panelSizes = PANEL_SIZES;
	let documentPanel: Panel | undefined;

	$: documentPanel?.scrollTabIntoView($portfolio.activeDocumentIndex);

	$: documentTabLabels = $portfolio.documents.map((doc: FrontendDocumentDetails) => {
		const name = doc.displayName;

		if (!editor.instance.inDevelopmentMode()) return { name };

		const tooltip = `Document ID: ${doc.id}`;
		return { name, tooltip };
	});

	const editor = getContext<Editor>("editor");
	const portfolio = getContext<PortfolioState>("portfolio");
	const dialog = getContext<DialogState>("dialog");

	function resizePanel(e: PointerEvent) {
		const gutter = (e.target || undefined) as HTMLDivElement | undefined;
		const nextSibling = (gutter?.nextElementSibling || undefined) as HTMLDivElement | undefined;
		const prevSibling = (gutter?.previousElementSibling || undefined) as HTMLDivElement | undefined;
		const parentElement = (gutter?.parentElement || undefined) as HTMLDivElement | undefined;

		const nextSiblingName = (nextSibling?.getAttribute("data-subdivision-name") || undefined) as keyof typeof PANEL_SIZES;
		const prevSiblingName = (prevSibling?.getAttribute("data-subdivision-name") || undefined) as keyof typeof PANEL_SIZES;

		if (!gutter || !nextSibling || !prevSibling || !parentElement || !nextSiblingName || !prevSiblingName) return;

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
		gutter.setPointerCapture(e.pointerId);

		const mouseStart = isHorizontal ? e.clientX : e.clientY;

		const updatePosition = (e: PointerEvent) => {
			const mouseCurrent = isHorizontal ? e.clientX : e.clientY;
			let mouseDelta = mouseStart - mouseCurrent;

			mouseDelta = Math.max(nextSiblingSize + mouseDelta, MIN_PANEL_SIZE) - nextSiblingSize;
			mouseDelta = prevSiblingSize - Math.max(prevSiblingSize - mouseDelta, MIN_PANEL_SIZE);

			panelSizes[nextSiblingName] = ((nextSiblingSize + mouseDelta) / totalResizingSpaceOccupied) * proportionBeingResized * 100;
			panelSizes[prevSiblingName] = ((prevSiblingSize - mouseDelta) / totalResizingSpaceOccupied) * proportionBeingResized * 100;

			window.dispatchEvent(new CustomEvent("resize"));
		};

		const cleanup = (e: PointerEvent) => {
			gutter.releasePointerCapture(e.pointerId);

			document.removeEventListener("pointermove", updatePosition);
			document.removeEventListener("pointerleave", cleanup);
			document.removeEventListener("pointerup", cleanup);
		};

		document.addEventListener("pointermove", updatePosition);
		document.addEventListener("pointerleave", cleanup);
		document.addEventListener("pointerup", cleanup);
	}
</script>

<LayoutRow class="workspace" data-workspace>
	<LayoutRow class="workspace-grid-subdivision" styles={{ "flex-grow": panelSizes["root"] }} data-subdivision-name="root">
		<LayoutCol class="workspace-grid-subdivision" styles={{ "flex-grow": panelSizes["content"] }} data-subdivision-name="content">
			<LayoutRow class="workspace-grid-subdivision" styles={{ "flex-grow": panelSizes["document"] }} data-subdivision-name="document">
				<Panel
					panelType={$portfolio.documents.length > 0 ? "Document" : undefined}
					tabCloseButtons={true}
					tabMinWidths={true}
					tabLabels={documentTabLabels}
					clickAction={(tabIndex) => editor.instance.selectDocument($portfolio.documents[tabIndex].id)}
					closeAction={(tabIndex) => editor.instance.closeDocumentWithConfirmation($portfolio.documents[tabIndex].id)}
					tabActiveIndex={$portfolio.activeDocumentIndex}
					bind:this={documentPanel}
				/>
			</LayoutRow>
		</LayoutCol>
		<LayoutCol class="workspace-grid-resize-gutter" data-gutter-horizontal on:pointerdown={(e) => resizePanel(e)} />
		<LayoutCol class="workspace-grid-subdivision" styles={{ "flex-grow": panelSizes["details"] }} data-subdivision-name="details">
			<LayoutRow class="workspace-grid-subdivision" styles={{ "flex-grow": panelSizes["properties"] }} data-subdivision-name="properties">
				<Panel panelType="Properties" tabLabels={[{ name: "Properties" }]} tabActiveIndex={0} />
			</LayoutRow>
			<LayoutRow class="workspace-grid-resize-gutter" data-gutter-vertical on:pointerdown={(e) => resizePanel(e)} />
			<LayoutRow class="workspace-grid-subdivision" styles={{ "flex-grow": panelSizes["layers"] }} data-subdivision-name="layers">
				<Panel panelType="Layers" tabLabels={[{ name: "Layers" }]} tabActiveIndex={0} />
			</LayoutRow>
		</LayoutCol>
	</LayoutRow>
	{#if $dialog.visible}
		<Dialog />
	{/if}
</LayoutRow>

<style lang="scss" global>
	.workspace {
		position: relative;
		flex: 1 1 100%;

		.workspace-grid-subdivision {
			min-height: 28px;
			flex: 1 1 0;

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
	}
</style>

<script lang="ts">
	import { getContext } from "svelte";

	import { fade } from "svelte/transition";

	import type { DialogState } from "@graphite/state-providers/dialog";
	import { type DockspaceState, type PanelIdentifier, type PanelDragging } from "@graphite/state-providers/dockspace";

	import type { Editor } from "@graphite/wasm-communication/editor";

	import type { Direction } from "@graphite/wasm-communication/messages";

	import Dialog from "@graphite/components/floating-menus/Dialog.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import SubdivisionOrPanel from "@graphite/components/window/workspace/SubdivisionOrPanel.svelte";

	const dialog = getContext<DialogState>("dialog");
	const editor = getContext<Editor>("editor");
	const dockspace = getContext<DockspaceState>("dockspace");

	let dragTarget = undefined as undefined | DOMRect;
	let dragState = undefined as undefined | { edge: Edge; dragging: PanelDragging; insert: InsertIndex | undefined; bodyId: PanelIdentifier | undefined; tabsId: PanelIdentifier | undefined };

	function panelBody(e: DragEvent): undefined | { id: PanelIdentifier; element: HTMLElement } {
		if (!(e.target instanceof Element)) return;
		const element = e.target.closest("[data-panel-body]");
		if (element === null || !(element instanceof HTMLElement)) return;
		if (element.dataset.panelBody === undefined) return;
		return { id: BigInt(element.dataset.panelBody), element: element };
	}
	function panelTabs(e: DragEvent): undefined | { id: PanelIdentifier; element: HTMLElement } {
		if (!(e.target instanceof Element)) return;
		const element = e.target.closest("[data-panel-tabs]");
		if (element === null || !(element instanceof HTMLElement)) return;
		if (element.dataset.panelTabs === undefined) return;
		return { id: BigInt(element.dataset.panelTabs), element };
	}

	type Edge = { direction: Direction; position: bigint } | undefined;
	function onEdge(e: DragEvent, element: HTMLElement): Edge {
		const bounds = element.getBoundingClientRect();
		const fractionX = (e.clientX - bounds.x) / bounds.width;
		const fractionY = (e.clientY - bounds.y) / bounds.height;
		const TARGET_FRACTION = 0.2;

		if (fractionX < Math.min(TARGET_FRACTION, fractionY, 1 - fractionY)) return { direction: "Horizontal", position: 0n };
		if (1 - fractionX < Math.min(TARGET_FRACTION, fractionY, 1 - fractionY)) return { direction: "Horizontal", position: 1n };
		if (fractionY < Math.min(TARGET_FRACTION)) return { direction: "Vertical", position: 0n };
		if (1 - fractionY < Math.min(TARGET_FRACTION)) return { direction: "Vertical", position: 1n };
	}

	function canDockEdge(id: PanelIdentifier) {
		return !(id === $dockspace.panelDragging?.panel && editor.handle.isSingleTab(id));
	}

	type InsertIndex = { x: number; y: number; insertAtIndex: number; passedActive: boolean };
	function insertIndex(e: DragEvent, element: HTMLElement): InsertIndex {
		let { x, y } = element.getBoundingClientRect();

		let insertAtIndex = 0;
		let passedActive = false;
		element.childNodes.forEach((tab) => {
			if (!(tab instanceof HTMLElement)) return;
			const bounds = tab.getBoundingClientRect();
			if (bounds.x + bounds.width / 2 > e.clientX) return;
			const index = Number(tab.dataset.tabIndex);
			insertAtIndex = index + 1;
			x = bounds.right;
			y = bounds.y;
		});
		return { insertAtIndex, passedActive, x, y };
	}

	function dragover(e: DragEvent) {
		if (!$dockspace.panelDragging) return;
		const tabs = panelTabs(e);
		const body = panelBody(e);
		dragState = { edge: undefined, dragging: $dockspace.panelDragging, bodyId: body?.id, tabsId: tabs?.id, insert: undefined };
		if (tabs !== undefined) {
			dragTarget = undefined;

			const insert = insertIndex(e, tabs.element);
			dragState.insert = insert;
		} else if (body !== undefined) {
			dragTarget = body.element.getBoundingClientRect();

			const edge = onEdge(e, body.element);
			if (canDockEdge(body.id) && edge !== undefined) {
				dragState.edge = edge;
				if (edge.direction === "Horizontal") dragTarget.width /= 2;
				else if (edge.direction === "Vertical") dragTarget.height /= 2;

				if (edge.direction === "Horizontal" && edge.position === 1n) dragTarget.x += dragTarget.width;
				else if (edge.direction === "Vertical" && edge.position === 1n) dragTarget.y += dragTarget.height;
			}
		}
	}

	function dragend() {
		if (dragState === undefined) return;
		const previousDragState = dragState;
		dragState = undefined;

		const target = previousDragState.tabsId === undefined ? previousDragState.bodyId : previousDragState.tabsId;
		if (target === undefined) return;

		let insertIndex = undefined;
		if (previousDragState.insert !== undefined && previousDragState.tabsId !== undefined) {
			const indexOffset = target === previousDragState.dragging.panel && previousDragState.dragging.tabIndex < previousDragState.insert.insertAtIndex ? 1 : 0;
			insertIndex = previousDragState.insert.insertAtIndex - indexOffset;
		}

		let horizontal = undefined;
		let start = undefined;
		if (previousDragState.edge) {
			horizontal = previousDragState.edge.direction === "Horizontal";
			start = previousDragState.edge.position === 0n;
		}

		editor.handle.moveTab(previousDragState.dragging.panel, previousDragState.dragging.tabIndex, target, insertIndex, horizontal, start);
	}
</script>

<LayoutRow class="workspace" data-workspace on:dragover={dragover} on:dragend={dragend} ve={() => (dragTarget = undefined)}>
	{#if $dockspace.divisionData !== undefined}
		<SubdivisionOrPanel value={$dockspace.divisionData} />
	{/if}
	{#if $dialog.visible}
		<Dialog />
	{/if}
</LayoutRow>
{#if $dockspace.panelDragging !== undefined && dragTarget !== undefined}
	<div class="drag-target" transition:fade={{ duration: 150 }} style={`left:${dragTarget.x}px;top:${dragTarget.y}px;width:${dragTarget.width}px;height:${dragTarget.height}px;`} />
{/if}
{#if $dockspace.panelDragging !== undefined && dragState?.insert !== undefined}
	<div class="insert-target" style={`left:${dragState.insert.x}px;top:${dragState.insert.y}px;`} />
{/if}

<style lang="scss" global>
	.workspace {
		position: relative;
		flex: 1 1 100%;
	}
	.drag-target {
		pointer-events: none;
		position: absolute;
		background-color: rgba(170, 170, 170, 0.2);
		transition: all 0.15s ease-out;
	}
	.insert-target {
		pointer-events: none;
		position: absolute;
		background-color: white;
		width: 2px;
		height: 30px;
		border-radius: 1px;
	}
</style>

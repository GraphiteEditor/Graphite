import { writable } from "svelte/store";
import type { EditorWrapper } from "/wrapper/pkg/graphite_wasm_wrapper";

// Wire dragging between the node graph and the input connectors shown in the Properties panel.
// The graph's pointer capture hides hover from the panel's DOM, so the connector under the pointer is found by hit-testing instead.

export type PropertiesPanelConnector = { nodeId: bigint; inputIndex: number };

const DRAG_OUT_THRESHOLD = 4;

// The Properties panel connector that a wire dragged from the graph is hovering, which renders as a connector awaiting the drop
export const hoveredWireDropTarget = writable<PropertiesPanelConnector | undefined>(undefined);

let pressedConnector: (PropertiesPanelConnector & { element: Element; startX: number; startY: number }) | undefined = undefined;
let draggingOutOfPropertiesPanel = false;
let suppressNextClick = false;

let reportedHoveringPanel = false;
let reportedTarget: PropertiesPanelConnector | undefined = undefined;

export function pressPropertiesPanelConnector(e: PointerEvent, connector: PropertiesPanelConnector) {
	if (e.button !== 0 || !(e.currentTarget instanceof Element)) return;

	pressedConnector = { ...connector, element: e.currentTarget, startX: e.clientX, startY: e.clientY };
	suppressNextClick = false;
}

// Whether the click ending a press should be ignored because the press became a wire drag
export function consumeConnectorClickSuppression(): boolean {
	const suppress = suppressNextClick;
	suppressNextClick = false;
	return suppress;
}

// Returns true when this movement begins dragging a wire out of a Properties panel connector, which makes it a pointer interaction the editor must receive
export function wireDragPointerMove(e: PointerEvent, editor: EditorWrapper, graphViewOverlayOpen: boolean, interactionOngoing: boolean): boolean {
	let started = false;

	// Begin dragging out of a pressed connector once the pointer has moved far enough that it's no longer a click
	if (pressedConnector && !draggingOutOfPropertiesPanel && e.buttons) {
		const distance = Math.hypot(e.clientX - pressedConnector.startX, e.clientY - pressedConnector.startY);
		if (distance > DRAG_OUT_THRESHOLD && graphViewOverlayOpen) {
			const [x, y] = elementCenter(pressedConnector.element);
			editor.startWireFromPropertiesPanel(pressedConnector.nodeId, pressedConnector.inputIndex, x, y);

			draggingOutOfPropertiesPanel = true;
			reportedHoveringPanel = true;
			started = true;
		}
	}

	if (!graphViewOverlayOpen || !e.buttons || !(interactionOngoing || started)) return started;

	// Find which Properties panel connector, if any, is under the pointer
	const topmost = document.elementFromPoint(e.clientX, e.clientY);
	// The whole panel counts, including its tab bar, so crossing the tabs doesn't pan the graph
	const hoveringPanel = Boolean(topmost?.closest("[data-panel-body]")?.querySelector("[data-properties-panel]"));
	const targetElement = topmost?.closest("[data-wire-drop-target]") || undefined;
	const target = targetElement && connectorFromElement(targetElement);

	if (hoveringPanel) {
		const dropTargets = Array.from(document.querySelectorAll("[data-wire-drop-target]")).map((element) => ({ element, bounds: element.getBoundingClientRect() }));
		// eslint-disable-next-line no-console
		console.debug("[wire-drag] Hit test", { pointer: [e.clientX, e.clientY], topmost, elementsAtPoint: document.elementsFromPoint(e.clientX, e.clientY), dropTargets });
	}

	const targetChanged = target?.nodeId !== reportedTarget?.nodeId || target?.inputIndex !== reportedTarget?.inputIndex;
	if (hoveringPanel === reportedHoveringPanel && !targetChanged) return started;

	reportedHoveringPanel = hoveringPanel;
	reportedTarget = target;
	hoveredWireDropTarget.set(target);

	const [x, y] = targetElement ? elementCenter(targetElement) : [0, 0];
	// eslint-disable-next-line no-console
	console.debug("[wire-drag] Reporting hover", { hoveringPanel, target, topmost, dropTargetsInDocument: document.querySelectorAll("[data-wire-drop-target]").length });
	editor.setWirePropertiesPanelHover(hoveringPanel, target?.nodeId, target?.inputIndex ?? 0, x, y);

	return started;
}

export function wireDragPointerUp(e: PointerEvent, editor: EditorWrapper) {
	if (e.buttons) return;

	// The editor never saw the pointer go down in the Properties panel, so it needs to be told about the release
	if (draggingOutOfPropertiesPanel) {
		editor.releaseWireFromPropertiesPanel();
		suppressNextClick = true;
	}

	pressedConnector = undefined;
	draggingOutOfPropertiesPanel = false;
	reportedHoveringPanel = false;
	reportedTarget = undefined;
	hoveredWireDropTarget.set(undefined);
}

function connectorFromElement(element: Element): PropertiesPanelConnector | undefined {
	const nodeId = element.getAttribute("data-node-id") || undefined;
	const inputIndex = element.getAttribute("data-input-index") || undefined;
	if (nodeId === undefined || inputIndex === undefined) return undefined;

	return { nodeId: BigInt(nodeId), inputIndex: Number(inputIndex) };
}

function elementCenter(element: Element): [number, number] {
	const bounds = element.getBoundingClientRect();
	return [bounds.left + bounds.width / 2, bounds.top + bounds.height / 2];
}

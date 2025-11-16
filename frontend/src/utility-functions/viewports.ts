import { type Editor } from "@graphite/editor";

export function updateBoundsOfViewports(editor: Editor) {
	const viewports = Array.from(window.document.querySelectorAll("[data-viewport-container]"));

	if (viewports.length <= 0) return;

	const bounds = viewports[0].getBoundingClientRect();
	const scale = window.devicePixelRatio || 1;

	editor.handle.updateViewport(bounds.x, bounds.y, bounds.width, bounds.height, scale);
}

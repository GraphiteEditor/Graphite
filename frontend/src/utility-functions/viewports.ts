import { type Editor } from "@graphite/editor";

export function updateBoundsOfViewports(editor: Editor) {
	const viewports = Array.from(window.document.querySelectorAll("[data-viewport-container]"));
	const boundsOfViewports = viewports.map((canvas) => {
		const bounds = canvas.getBoundingClientRect();
		return [bounds.left, bounds.top, bounds.right, bounds.bottom];
	});

	const flattened = boundsOfViewports.flat();
	const data = Float64Array.from(flattened);

	if (boundsOfViewports.length > 0) editor.handle.boundsOfViewports(data);
}

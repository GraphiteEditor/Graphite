import { type Editor } from "@graphite/editor";

export function updateBoundsOfViewports(editor: Editor, container: HTMLElement) {
	const viewports = Array.from(container.querySelectorAll("[data-viewport]"));
	const boundsOfViewports = viewports.map((canvas) => {
		const bounds = canvas.getBoundingClientRect();
		return [bounds.left, bounds.top, bounds.right, bounds.bottom];
	});

	const flattened = boundsOfViewports.flat();
	const data = Float64Array.from(flattened);

	if (boundsOfViewports.length > 0) editor.handle.boundsOfViewports(data);
}

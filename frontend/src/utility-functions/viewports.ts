import { type Editor } from "@graphite/editor";

export function updateBoundsOfViewports(editor: Editor) {
	const viewports = Array.from(window.document.querySelectorAll("[data-viewport-container]"));

	// Get device pixel ratio to scale bounds for high-DPI devices like iPad
	const dpr = window.devicePixelRatio || 1;

	const boundsOfViewports = viewports.map((canvas) => {
		const bounds = canvas.getBoundingClientRect();
		// Scale bounds by device pixel ratio to match scaled pointer coordinates
		return [bounds.left * dpr, bounds.top * dpr, bounds.right * dpr, bounds.bottom * dpr];
	});

	const flattened = boundsOfViewports.flat();
	const data = Float64Array.from(flattened);

	if (boundsOfViewports.length > 0) editor.handle.boundsOfViewports(data);
}

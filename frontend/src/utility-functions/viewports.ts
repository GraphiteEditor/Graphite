import { type Editor } from "@graphite/editor";

let resizeObserver: ResizeObserver | undefined;

export function setupViewportResizeObserver(editor: Editor) {
	// Clean up existing observer if any
	if (resizeObserver) {
		resizeObserver.disconnect();
	}

	const viewports = Array.from(window.document.querySelectorAll("[data-viewport-container]"));
	if (viewports.length <= 0) return;

	const viewport = viewports[0] as HTMLElement;

	resizeObserver = new ResizeObserver((entries) => {
		for (const entry of entries) {
			const devicePixelRatio = window.devicePixelRatio || 1;

			// Get exact device pixel dimensions from the browser
			// Use devicePixelContentBoxSize for pixel-perfect rendering with fallback for Safari
			let physicalWidth: number;
			let physicalHeight: number;

			if (entry.devicePixelContentBoxSize && entry.devicePixelContentBoxSize.length > 0) {
				// Modern browsers (Chrome, Firefox): get exact device pixels from the browser
				physicalWidth = entry.devicePixelContentBoxSize[0].inlineSize;
				physicalHeight = entry.devicePixelContentBoxSize[0].blockSize;
			} else {
				// Fallback for Safari: calculate from contentBoxSize and devicePixelRatio
				physicalWidth = entry.contentBoxSize[0].inlineSize * devicePixelRatio;
				physicalHeight = entry.contentBoxSize[0].blockSize * devicePixelRatio;
			}

			// Compute the logical size which corresponds to the physical size
			const logicalWidth = physicalWidth / devicePixelRatio;
			const logicalHeight = physicalHeight / devicePixelRatio;

			// Get viewport position
			const bounds = entry.target.getBoundingClientRect();

			// TODO: Consider passing physical sizes as well to eliminate pixel inaccuracies since width and height could be rounded differently
			const scale = physicalWidth / logicalWidth;

			if (!scale || scale <= 0) {
				continue;
			}

			editor.handle.updateViewport(bounds.x, bounds.y, logicalWidth, logicalHeight, scale);
		}
	});

	resizeObserver.observe(viewport);
}

export function cleanupViewportResizeObserver() {
	if (resizeObserver) {
		resizeObserver.disconnect();
		resizeObserver = undefined;
	}
}

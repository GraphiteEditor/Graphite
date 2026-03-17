document.addEventListener("DOMContentLoaded", () => {
	const container = document.querySelector(".crate-hierarchy");
	if (!container) return;

	const svg = container.querySelector("svg");
	if (!svg) return;

	// Wrap SVG in a viewport container
	const viewport = document.createElement("div");
	viewport.className = "crate-hierarchy-viewport";
	svg?.parentNode?.insertBefore(viewport, svg);
	viewport.appendChild(svg);

	// Remove any width/height attributes so CSS controls sizing
	svg.removeAttribute("width");
	svg.removeAttribute("height");

	// Create zoom controls
	const controls = document.createElement("div");
	controls.className = "crate-hierarchy-controls";
	controls.innerHTML = `<button class="zoom-in"></button><button class="zoom-out"></button>`;
	container.insertBefore(controls, viewport);
	const zoomInBtn = controls.querySelector(".zoom-in");
	const zoomOutBtn = controls.querySelector(".zoom-out");
	if (!(zoomInBtn instanceof HTMLButtonElement) || !(zoomOutBtn instanceof HTMLButtonElement)) return;

	// Lock the viewport height to the SVG's natural rendered height (ignoring any zoom transform)
	const updateViewportHeight = () => {
		const prevTransform = svg.style.transform;
		svg.style.transform = "";
		viewport.style.height = `${svg.getBoundingClientRect().height}px`;
		svg.style.transform = prevTransform;
	};
	updateViewportHeight();
	window.addEventListener("resize", () => {
		updateViewportHeight();
		applyTransform();
	});

	const MIN_SCALE = 1;
	const MAX_SCALE = 4;
	const ZOOM_STEP = 0.15;
	const BUTTON_ZOOM_STEP = 0.5;
	const ANIMATION_DURATION = 200;

	let scale = MIN_SCALE;
	let panX = 0;
	let panY = 0;
	let animationFrameId = 0;
	let isDragging = false;
	let dragStartX = 0;
	let dragStartY = 0;
	let panStartX = 0;
	let panStartY = 0;

	function clampPan() {
		const viewportRect = viewport.getBoundingClientRect();
		const viewportW = viewportRect.width;
		const viewportH = viewportRect.height;

		// The SVG is scaled to fill the viewport width at scale=1
		const scaledW = viewportW * scale;
		const scaledH = svg?.getBoundingClientRect()?.height || 0;

		// How much overflow exists on each axis
		const overflowX = Math.max(0, scaledW - viewportW);
		const overflowY = Math.max(0, scaledH - viewportH);

		// Pan is constrained so scaled content edges don't pull away from viewport edges
		panX = Math.min(0, Math.max(-overflowX, panX));
		panY = Math.min(0, Math.max(-overflowY, panY));
	}

	function updateButtons() {
		if (zoomInBtn instanceof HTMLButtonElement) zoomInBtn.disabled = scale >= MAX_SCALE;
		if (zoomOutBtn instanceof HTMLButtonElement) zoomOutBtn.disabled = scale <= MIN_SCALE;
	}

	function applyTransform() {
		clampPan();
		if (svg) svg.style.transform = `translate(${panX}px, ${panY}px) scale(${scale})`;
		updateButtons();
	}

	function zoomAt(/** @type {number} */ clientX, /** @type {number} */ clientY, /** @type {number} */ newScale) {
		const viewportRect = viewport.getBoundingClientRect();

		// Point in viewport-local coordinates
		const pointX = clientX - viewportRect.left;
		const pointY = clientY - viewportRect.top;

		// Where this point maps in the pre-zoom content
		const contentX = (pointX - panX) / scale;
		const contentY = (pointY - panY) / scale;

		scale = Math.min(MAX_SCALE, Math.max(MIN_SCALE, newScale));

		// Adjust pan so the same content point stays under the cursor
		panX = pointX - contentX * scale;
		panY = pointY - contentY * scale;

		applyTransform();
	}

	function animateZoomAt(/** @type {number} */ clientX, /** @type {number} */ clientY, /** @type {number} */ newTargetScale) {
		cancelAnimationFrame(animationFrameId);

		const targetScale = Math.min(MAX_SCALE, Math.max(MIN_SCALE, newTargetScale));
		const startScale = scale;
		const startPanX = panX;
		const startPanY = panY;

		const viewportRect = viewport.getBoundingClientRect();
		const pointX = clientX - viewportRect.left;
		const pointY = clientY - viewportRect.top;
		const contentX = (pointX - panX) / scale;
		const contentY = (pointY - panY) / scale;

		const targetPanX = pointX - contentX * targetScale;
		const targetPanY = pointY - contentY * targetScale;

		const startTime = performance.now();
		const step = (/** @type {number} */ now) => {
			const t = Math.min(1, (now - startTime) / ANIMATION_DURATION);
			const ease = t * (2 - t); // ease-out quadratic
			scale = startScale + (targetScale - startScale) * ease;
			panX = startPanX + (targetPanX - startPanX) * ease;
			panY = startPanY + (targetPanY - startPanY) * ease;
			applyTransform();
			if (t < 1) animationFrameId = requestAnimationFrame(step);
		};
		animationFrameId = requestAnimationFrame(step);
	}

	// Scroll wheel zoom
	viewport.addEventListener(
		"wheel",
		(e) => {
			e.preventDefault();
			const delta = e.deltaY > 0 ? -ZOOM_STEP : ZOOM_STEP;
			zoomAt(e.clientX, e.clientY, scale + delta);
		},
		{ passive: false },
	);

	// Button zoom (animated, zoom toward center of viewport)
	zoomInBtn?.addEventListener("click", () => {
		const rect = viewport.getBoundingClientRect();
		animateZoomAt(rect.left + rect.width / 2, rect.top + rect.height / 2, scale + BUTTON_ZOOM_STEP);
	});
	zoomOutBtn?.addEventListener("click", () => {
		const rect = viewport.getBoundingClientRect();
		animateZoomAt(rect.left + rect.width / 2, rect.top + rect.height / 2, scale - BUTTON_ZOOM_STEP);
	});

	// Click-drag to pan
	viewport.addEventListener("pointerdown", (e) => {
		if (e.button !== 0) return;
		e.preventDefault();
		isDragging = true;
		dragStartX = e.clientX;
		dragStartY = e.clientY;
		panStartX = panX;
		panStartY = panY;
		viewport.setPointerCapture(e.pointerId);
		viewport.style.cursor = "grabbing";
	});
	window.addEventListener("pointermove", (e) => {
		if (!isDragging) return;
		panX = panStartX + (e.clientX - dragStartX);
		panY = panStartY + (e.clientY - dragStartY);
		applyTransform();
	});
	window.addEventListener("pointerup", () => {
		if (!isDragging) return;
		isDragging = false;
		viewport.style.cursor = "";
	});

	applyTransform();
});

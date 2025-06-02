const RECENTER_DELAY = 1;
const RECENTER_ANIMATION_DURATION = 0.25;

window.addEventListener("DOMContentLoaded", initializeImageComparison);

function initializeImageComparison() {
	Array.from(document.querySelectorAll("[data-image-comparison]")).forEach((element) => {
		const moveHandler = (event) => {
			const factor = (event.clientX - element.getBoundingClientRect().left) / element.getBoundingClientRect().width;
			const capped = Math.max(0, Math.min(1, factor));

			if (!(element instanceof HTMLElement)) return;
			element.style.setProperty("--comparison-percent", `${capped * 100}%`);
			element.dataset.lastInteraction = "";
		};

		const leaveHandler = (event) => {
			moveHandler(event);

			const randomCode = Math.random().toString().substring(2);
			element.dataset.lastInteraction = randomCode;

			setTimeout(() => {
				if (element.dataset.lastInteraction === randomCode) {
					element.dataset.recenterStartTime = Date.now();
					element.dataset.recenterStartValue = parseFloat(element.style.getPropertyValue("--comparison-percent"));

					recenterAnimationStep();
				}
			}, RECENTER_DELAY * 1000);
		};

		const recenterAnimationStep = () => {
			if (element.dataset.lastInteraction === "") return;

			const completionFactor = (Date.now() - element.dataset.recenterStartTime) / (RECENTER_ANIMATION_DURATION * 1000);
			if (completionFactor > 1) {
				element.dataset.lastInteraction = "";
				return;
			}

			const factor = smootherstep(completionFactor);
			const newLocation = lerp(element.dataset.recenterStartValue, 50, factor);
			element.style.setProperty("--comparison-percent", `${newLocation}%`);

			requestAnimationFrame(recenterAnimationStep);
		};

		const lerp = (a, b, t) => (1 - t) * a + t * b;
		const smootherstep = (x) => x * x * x * (x * (x * 6 - 15) + 10);

		element.addEventListener("pointermove", moveHandler);
		element.addEventListener("pointerenter", moveHandler);
		element.addEventListener("pointerleave", leaveHandler);
		element.addEventListener("dragstart", (event) => event.preventDefault());
	});
}

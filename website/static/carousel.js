const FLING_VELOCITY_THRESHOLD = 10;
const FLING_VELOCITY_WINDOW_SIZE = 20;

let carouselImages;
let carouselDirectionPrev;
let carouselDirectionNext;
let carouselDots;
let carouselDescriptions;
let carouselDragLastClientX;
let velocityDeltaWindow = Array.from({ length: FLING_VELOCITY_WINDOW_SIZE }, () => ({ time: 0, delta: 0 }));

window.addEventListener("DOMContentLoaded", initializeCarousel);
window.addEventListener("pointerup", () => dragEnd(false));
window.addEventListener("scroll", () => dragEnd(true));
window.addEventListener("pointermove", dragMove);

function initializeCarousel() {
	carouselImages = document.querySelectorAll(".carousel img");
	carouselImages.forEach((image) => {
		image.addEventListener("pointerdown", dragBegin);
	});

	carouselDirectionPrev = document.querySelector(".carousel-controls .direction.prev");
	carouselDirectionNext = document.querySelector(".carousel-controls .direction.next");
	carouselDots = document.querySelectorAll(".carousel-controls .dot");
	carouselDescriptions = document.querySelectorAll(".screenshot-description p");

	carouselDirectionPrev.addEventListener("click", () => slideDirection("prev", false, true));
	carouselDirectionNext.addEventListener("click", () => slideDirection("next", false, true));
	Array.from(carouselDots).forEach((dot) => dot.addEventListener("click", (event) => {
		const index = Array.from(carouselDots).indexOf(event.target);
		slideTo(index, true);
	}));
}

function slideDirection(direction, clamped = false, smooth) {
	const directionIndexOffset = { prev: -1, next: 1 }[direction];
	const offsetDotIndex = currentClosestImageIndex() + directionIndexOffset;

	const nextDotIndex = (offsetDotIndex + carouselDots.length) % carouselDots.length;
	const unwrappedNextDotIndex = clamp(offsetDotIndex, 0, carouselDots.length - 1);

	if (clamped) slideTo(unwrappedNextDotIndex, smooth);
	else slideTo(nextDotIndex, smooth);
}

function slideTo(index, smooth) {
	const activeDot = document.querySelector(".carousel-controls .dot.active");
	activeDot.classList.remove("active");
	carouselDots[index].classList.add("active");

	const activeDescription = document.querySelector(".screenshot-description p.active");
	activeDescription.classList.remove("active");
	carouselDescriptions[index].classList.add("active");

	setCurrentTransform(index * -100, "%", smooth)
}

function currentTransform() {
	const currentTransformMatrix = window.getComputedStyle(carouselImages[0]).transform;
	// Grab the X value from the format that looks like: `matrix(1, 0, 0, 1, -1332.13, 0)` or `none`
	return Number(currentTransformMatrix.split(",")[4] || "0");
}

function setCurrentTransform(x, unit, smooth) {
	Array.from(carouselImages).forEach((image) => {
		image.style.transitionTimingFunction = smooth ? "ease-in-out" : "cubic-bezier(0, 0, 0.2, 1)";
		image.style.transform = `translateX(${x}${unit})`;
	});
}

function currentClosestImageIndex() {
	const currentTransformX = -currentTransform();

	const imageWidth = carouselImages[0].getBoundingClientRect().width;
	return Math.round(currentTransformX / imageWidth);
}

function currentActiveDotIndex() {
	const activeDot = document.querySelector(".carousel-controls .dot.active");
	return Array.from(carouselDots).indexOf(activeDot);
}

function dragBegin(event) {
	event.preventDefault();

	carouselDragLastClientX = event.clientX;

	setCurrentTransform(currentTransform(), "px", false);
	document.querySelector("#screenshots").classList.add("dragging");
}

function dragEnd(dropWithoutVelocity) {
	if (!carouselImages) return;

	carouselDragLastClientX = undefined;

	document.querySelector("#screenshots").classList.remove("dragging");

	const onlyRecentVelocityDeltaWindow = velocityDeltaWindow.filter((delta) => delta.time > Date.now() - 1000);
	const timeRange = Date.now() - onlyRecentVelocityDeltaWindow[0]?.time;
	// Weighted (higher by recency) sum of velocity deltas from previous window of frames
	const recentVelocity = onlyRecentVelocityDeltaWindow.reduce((acc, entry) => {
		const timeSinceNow = Date.now() - entry.time;
		const recencyFactorScore = 1 - (timeSinceNow / timeRange);

		return acc + entry.delta * recencyFactorScore;
	}, 0);

	const closestImageIndex = currentClosestImageIndex();
	const activeDotIndex = currentActiveDotIndex();

	// If the speed is fast enough, slide to the next or previous image in that direction
	if (Math.abs(recentVelocity) > FLING_VELOCITY_THRESHOLD && !dropWithoutVelocity) {
		// Positive velocity should go to the previous image
		if (recentVelocity > 0) {
			// Don't apply the velocity-based fling if we're already snapping to the next image
			if (closestImageIndex >= activeDotIndex) {
				slideDirection("prev", true, false);
				return;
			}
		}
		// Negative velocity should go to the next image
		else {
			// Don't apply the velocity-based fling if we're already snapping to the next image
			if (closestImageIndex <= activeDotIndex) {
				slideDirection("next", true, false);
				return;
			}
		}
	}

	// If we didn't slide in a direction due to clear velocity, just snap to the closest image
	// This can be reached either by not entering the if statement above, or by its inner if statements not returning early and exiting back to this scope
	slideTo(clamp(closestImageIndex, 0, carouselDots.length - 1), true);
}

function dragMove(event) {
	if (carouselDragLastClientX === undefined) return;

	event.preventDefault();

	const LEFT_MOUSE_BUTTON = 1;
	if (!(event.buttons & LEFT_MOUSE_BUTTON)) {
		dragEnd(false);
		return;
	}

	const deltaX = event.clientX - carouselDragLastClientX;
	velocityDeltaWindow.shift();
	velocityDeltaWindow.push({ time: Date.now(), delta: deltaX });

	const newTransformX = currentTransform() + deltaX;
	setCurrentTransform(newTransformX, "px", false);

	carouselDragLastClientX = event.clientX;
}

function clamp(value, min, max) {
	return Math.min(Math.max(value, min), max);
}

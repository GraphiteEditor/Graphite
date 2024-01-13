// ================================================================================================
// CAROUSEL
// ================================================================================================

const FLING_VELOCITY_THRESHOLD = 10;
const FLING_VELOCITY_WINDOW_SIZE = 20;

window.addEventListener("DOMContentLoaded", initializeCarousel);
window.addEventListener("pointerup", () => dragEnd(false));
window.addEventListener("scroll", () => dragEnd(true));
window.addEventListener("pointermove", dragMove);

const carousels = [];

function initializeCarousel() {
	const carouselContainers = document.querySelectorAll("[data-carousel]");

	carouselContainers.forEach((carouselContainer) => {
		const images = carouselContainer.querySelectorAll("[data-carousel-image]");
		const directionPrev = carouselContainer.querySelector("[data-carousel-prev]");
		const directionNext = carouselContainer.querySelector("[data-carousel-next]");
		const dots = carouselContainer.querySelectorAll("[data-carousel-dot]");
		const descriptions = carouselContainer.querySelectorAll("[data-carousel-description]");
		const performJostleHint = carouselContainer.hasAttribute("data-carousel-jostle-hint");
		const dragLastClientX = undefined;
		const velocityDeltaWindow = Array.from({ length: FLING_VELOCITY_WINDOW_SIZE }, () => ({ time: 0, delta: 0 }));
		const jostleNoLongerNeeded = false;

		const carousel = {
			carouselContainer,
			images,
			directionPrev,
			directionNext,
			dots,
			descriptions,
			dragLastClientX,
			velocityDeltaWindow,
			jostleNoLongerNeeded,
		};
		carousels.push(carousel);

		images.forEach((image) => {
			image.addEventListener("pointerdown", dragBegin);
		});
		directionPrev.addEventListener("click", () => slideDirection(carousel, "prev", true, false));
		directionNext.addEventListener("click", () => slideDirection(carousel, "next", true, false));
		Array.from(dots).forEach((dot) =>
			dot.addEventListener("click", (event) => {
				const index = Array.from(dots).indexOf(event.target);
				slideTo(carousel, index, true);
			})
		);

		// Jostle hint is a feature to briefly shift the carousel by a bit as a hint to users that it can be interacted with
		if (performJostleHint) {
			window.addEventListener("load", () => {
				new IntersectionObserver((entries) => {
					entries.forEach((entry) => {
						if (entry.intersectionRatio === 1 && currentTransform(carousel) === 0 && !carousel.jostleNoLongerNeeded) {
							const JOSTLE_TIME = 1000;
							const MAX_JOSTLE_DISTANCE = -10;

							let startTime;
							const buildUp = (timeStep) => {
								if (carousel.jostleNoLongerNeeded) return;

								if (!startTime) startTime = timeStep;
								const elapsedTime = timeStep - startTime;

								const easeOutCirc = (x) => Math.sqrt(1 - Math.pow(x - 1, 2));
								const movementFactor = easeOutCirc(Math.min(1, elapsedTime / JOSTLE_TIME));

								setCurrentTransform(carousel, movementFactor * MAX_JOSTLE_DISTANCE, "%", false, true);

								if (elapsedTime < JOSTLE_TIME) {
									requestAnimationFrame(buildUp);
								} else {
									carousel.jostleNoLongerNeeded = true;
									slideTo(carousel, 0, true);
								}
							};
							requestAnimationFrame(buildUp);
						};
					});
				}, { threshold: 1 })
					.observe(directionPrev);
			});
		}
	});
}

function slideDirection(carousel, direction, smooth, clamped = false) {
	const directionIndexOffset = { prev: -1, next: 1 }[direction];
	const offsetDotIndex = currentClosestImageIndex(carousel) + directionIndexOffset;

	const nextDotIndex = (offsetDotIndex + carousel.dots.length) % carousel.dots.length;
	const unwrappedNextDotIndex = clamp(offsetDotIndex, 0, carousel.dots.length - 1);

	if (clamped) slideTo(carousel, unwrappedNextDotIndex, smooth);
	else slideTo(carousel, nextDotIndex, smooth);
}

function slideTo(carousel, index, smooth) {
	const activeDot = carousel.carouselContainer.querySelector("[data-carousel-dot].active");
	activeDot.classList.remove("active");
	carousel.dots[index].classList.add("active");

	const activeDescription = carousel.carouselContainer.querySelector("[data-carousel-description].active");
	if (activeDescription) {
		activeDescription.classList.remove("active");
		carousel.descriptions[index].classList.add("active");
	}

	setCurrentTransform(carousel, index * -100, "%", smooth);
}

function currentTransform(carousel) {
	const currentTransformMatrix = window.getComputedStyle(carousel.images[0]).transform;
	// Grab the X value from the format that looks like: `matrix(1, 0, 0, 1, -1332.13, 0)` or `none`
	return Number(currentTransformMatrix.split(",")[4] || "0");
}

function setCurrentTransform(carousel, x, unit, smooth, doNotTerminateJostle = false) {
	const xInitial = currentTransform(carousel);
	
	Array.from(carousel.images).forEach((image) => {
		image.style.transitionTimingFunction = smooth ? "ease-in-out" : "cubic-bezier(0, 0, 0.2, 1)";
		image.style.transform = `translateX(${x}${unit})`;
	});

	// If the user caused the carousel to move, we can assume they know how to use it and don't need the jostle hint anymore
	if (!doNotTerminateJostle && x !== xInitial) carousel.jostleNoLongerNeeded = true;
}

function currentClosestImageIndex(carousel) {
	const currentTransformX = -currentTransform(carousel);

	const imageWidth = carousel.images[0].getBoundingClientRect().width;
	return Math.round(currentTransformX / imageWidth);
}

function currentActiveDotIndex(carousel) {
	const activeDot = carousel.carouselContainer.querySelector("[data-carousel-dot].active");
	return Array.from(carousel.dots).indexOf(activeDot);
}

function dragBegin(event) {
	const carouselContainer = event.target.closest("[data-carousel]");
	const carousel = carousels.find((carousel) => carousel.carouselContainer === carouselContainer);
	if (!carousel) return;
	
	event.preventDefault();

	carousel.dragLastClientX = event.clientX;

	setCurrentTransform(carousel, currentTransform(carousel), "px", false);
	carouselContainer.classList.add("dragging");
}

function dragEnd(dropWithoutVelocity) {
	const carousel = carousels.find((carousel) => carousel.dragLastClientX !== undefined);
	if (!carousel) return;
	
	if (!carousel.images) return;

	carousel.dragLastClientX = undefined;

	carousel.carouselContainer.classList.remove("dragging");

	const onlyRecentVelocityDeltaWindow = carousel.velocityDeltaWindow.filter((delta) => delta.time > Date.now() - 1000);
	const timeRange = Date.now() - (onlyRecentVelocityDeltaWindow[0]?.time ?? NaN);
	// Weighted (higher by recency) sum of velocity deltas from previous window of frames
	const recentVelocity = onlyRecentVelocityDeltaWindow.reduce((acc, entry) => {
		const timeSinceNow = Date.now() - entry.time;
		const recencyFactorScore = 1 - timeSinceNow / timeRange;

		return acc + entry.delta * recencyFactorScore;
	}, 0);

	const closestImageIndex = currentClosestImageIndex(carousel);
	const activeDotIndex = currentActiveDotIndex(carousel);

	// If the speed is fast enough, slide to the next or previous image in that direction
	if (Math.abs(recentVelocity) > FLING_VELOCITY_THRESHOLD && !dropWithoutVelocity) {
		// Positive velocity should go to the previous image
		if (recentVelocity > 0) {
			// Don't apply the velocity-based fling if we're already snapping to the next image
			if (closestImageIndex >= activeDotIndex) {
				slideDirection(carousel, "prev", false, true);
				return;
			}
		}
		// Negative velocity should go to the next image
		else {
			// Don't apply the velocity-based fling if we're already snapping to the next image
			// eslint-disable-next-line no-lonely-if
			if (closestImageIndex <= activeDotIndex) {
				slideDirection(carousel, "next", false, true);
				return;
			}
		}
	}

	// If we didn't slide in a direction due to clear velocity, just snap to the closest image
	// This can be reached either by not entering the if statement above, or by its inner if statements not returning early and exiting back to this scope
	slideTo(carousel, clamp(closestImageIndex, 0, carousel.dots.length - 1), true);
}

function dragMove(event) {
	const carouselContainer = event.target.closest("[data-carousel]");
	const carousel = carousels.find((carousel) => carousel.carouselContainer === carouselContainer);
	if (!carousel) return;

	if (carousel.dragLastClientX === undefined) return;

	event.preventDefault();

	const LEFT_MOUSE_BUTTON = 1;
	if (!(event.buttons & LEFT_MOUSE_BUTTON)) {
		dragEnd(false);
		return;
	}

	const deltaX = event.clientX - carousel.dragLastClientX;
	carousel.velocityDeltaWindow.shift();
	carousel.velocityDeltaWindow.push({ time: Date.now(), delta: deltaX });

	const newTransformX = currentTransform(carousel) + deltaX;
	setCurrentTransform(carousel, newTransformX, "px", false);

	carousel.dragLastClientX = event.clientX;
}

function clamp(value, min, max) {
	return Math.min(Math.max(value, min), max);
}

// ================================================================================================
// IMAGE COMPARISON
// ================================================================================================

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

// ================================================================================================
// AUTO-PLAYING VIDEO
// ================================================================================================
window.addEventListener("DOMContentLoaded", initializeVideoAutoPlay);

function initializeVideoAutoPlay() {
	const VISIBILITY_COVERAGE_FRACTION = 0.25;

	const players = document.querySelectorAll("[data-auto-play]");
	players.forEach((player) => {
		if (!(player instanceof HTMLVideoElement)) return;

		let loaded = false;
		
		new IntersectionObserver((entries) => {
			entries.forEach((entry) => {
				if (!loaded && entry.intersectionRatio > VISIBILITY_COVERAGE_FRACTION) {
					player.play();
					
					loaded = true;
				};
			});
		}, { threshold: VISIBILITY_COVERAGE_FRACTION })
		.observe(player);
	});
}

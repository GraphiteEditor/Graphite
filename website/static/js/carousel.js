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
		const slideImages = carouselContainer.querySelectorAll("[data-carousel-slide] [data-carousel-image]");
		const tornLeft = carouselContainer.querySelector("[data-carousel-slide-torn-left]");
		const tornRight = carouselContainer.querySelector("[data-carousel-slide-torn-right]");
		[tornLeft, tornRight].forEach((insertInsideElement) => {
			slideImages.forEach((image) => {
				const clonedImage = image.cloneNode(true);
				if (clonedImage instanceof HTMLImageElement) clonedImage.alt = "";
				insertInsideElement.insertAdjacentElement("beforeend", clonedImage);
			});
		});

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
			requestAnimationFrameActive: false,
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
								if (carousel.jostleNoLongerNeeded) {
									carousel.carouselContainer.classList.remove("jostling");
									return;
								}

								if (!startTime) startTime = timeStep;
								const elapsedTime = timeStep - startTime;

								const easeOutCirc = (x) => Math.sqrt(1 - Math.pow(x - 1, 2));
								const movementFactor = easeOutCirc(Math.min(1, elapsedTime / JOSTLE_TIME));

								setCurrentTransform(carousel, movementFactor * MAX_JOSTLE_DISTANCE, "%", false, true);

								if (elapsedTime < JOSTLE_TIME) {
									requestAnimationFrame(buildUp);
								} else {
									carousel.carouselContainer.classList.remove("jostling");
									carousel.jostleNoLongerNeeded = true;
									slideTo(carousel, 0, true);
								}
							};
							carousel.carouselContainer.classList.add("jostling")
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

	// Account for the first image being the faded out last image
	const offsetIndex = index + 1;
	const slideImages = Array.from(carousel.carouselContainer.querySelectorAll("[data-carousel-slide] [data-carousel-image]"));
	// Remove lazy loading from the adjacent images
	slideImages[clamp(offsetIndex - 2, 0, slideImages.length - 1)].removeAttribute("loading");
	slideImages[clamp(offsetIndex - 1, 0, slideImages.length - 1)].removeAttribute("loading");
	slideImages[clamp(offsetIndex, 0, slideImages.length - 1)].removeAttribute("loading");
	slideImages[clamp(offsetIndex + 1, 0, slideImages.length - 1)].removeAttribute("loading");
	slideImages[clamp(offsetIndex + 2, 0, slideImages.length - 1)].removeAttribute("loading");

	setCurrentTransform(carousel, index * -100, "%", smooth);
}

function currentTransform(carousel) {
	const currentTransformMatrix = window.getComputedStyle(carousel.images[1]).transform;
	// Grab the X value from the format that looks like either of: `matrix(1, 0, 0, 1, -1332.13, 0)` or `none`
	const xValue = Number(currentTransformMatrix.split(",")[4] || "0");

	return xValue + carousel.images[1].getBoundingClientRect().width;
}

function setCurrentTransform(carousel, x, unit, smooth, doNotTerminateJostle = false) {
	const xInitial = currentTransform(carousel);
	let xValue = x;
	if (unit === "%") xValue = x - 100;
	if (unit === "px") xValue = x - carousel.images[1].getBoundingClientRect().width;

	Array.from(carousel.images).forEach((image) => {
		image.style.transitionTimingFunction = smooth ? "ease-in-out" : "cubic-bezier(0, 0, 0.2, 1)";
		image.style.transform = `translateX(${xValue}${unit})`;
	});

	// If the user caused the carousel to move, we can assume they know how to use it and don't need the jostle hint anymore
	if (!doNotTerminateJostle && x - xInitial < 0.0001) carousel.jostleNoLongerNeeded = true;

	const distance = unit === "%" ? x : x / carousel.images[1].getBoundingClientRect().width * 100;
	const overSlidingLeft = distance > 0;
	const overSlidingRight = distance < (carousel.dots.length - 1) * -100;

	if ((overSlidingLeft || overSlidingRight) && !carousel.requestAnimationFrameActive) updateOverSlide(carousel);
}

function updateOverSlide(carousel) {
	const paddingLeft = parseInt(getComputedStyle(carousel.images[1]).paddingLeft);
	const paddingRight = parseInt(getComputedStyle(carousel.images[carousel.images.length - 2]).paddingRight);
	const slidLeftDistance = carousel.images[1].getBoundingClientRect().left + paddingLeft - carousel.images[1].parentElement.getBoundingClientRect().left;
	const slidRightDistance = -(carousel.images[carousel.images.length - 2].getBoundingClientRect().right - paddingRight - carousel.images[1].parentElement.getBoundingClientRect().right);
	const imageWidth = carousel.images[1].getBoundingClientRect().width;
	const overSlideFactor = Math.min(1, Math.max(0, (Math.max(slidLeftDistance, slidRightDistance) / imageWidth)));

	const images = carousel.images[0].closest("[data-carousel]").querySelectorAll("[data-carousel-image]:first-child, [data-carousel-image]:last-child");

	// Call again the next frame if we're still sliding past the edge
	if (overSlideFactor > 0) {
		images.forEach((image) => {
			image.style.setProperty("--over-slide-factor", overSlideFactor);
		});

		carousel.requestAnimationFrameActive = true;
		requestAnimationFrame(() => updateOverSlide(carousel));
	} else {
		images.forEach((image) => {
			image.style.removeProperty("--over-slide-factor");
		});

		carousel.requestAnimationFrameActive = false;
	}
}

function currentClosestImageIndex(carousel) {
	const currentTransformX = -currentTransform(carousel);

	const imageWidth = carousel.images[1].getBoundingClientRect().width;
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
	const m = Math; // This is a workaround for a bug in Zola's minifier
	return m.min(m.max(value, min), max);
}

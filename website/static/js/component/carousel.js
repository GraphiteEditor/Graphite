const FLING_VELOCITY_THRESHOLD = 10;
const FLING_VELOCITY_WINDOW_SIZE = 20;

window.addEventListener("DOMContentLoaded", initializeCarousel);
window.addEventListener("pointerup", () => dragEnd(false));
window.addEventListener("scroll", () => dragEnd(true));
window.addEventListener("pointermove", dragMove);

/**
 * @typedef {{
 *   carouselContainer: Element,
 *   images: NodeListOf<Element>,
 *   directionPrev: Element | null,
 *   directionNext: Element | null,
 *   dots: NodeListOf<Element>,
 *   descriptions: NodeListOf<Element>,
 *   dragLastClientX: number | undefined,
 *   velocityDeltaWindow: Array<{ time: number, delta: number }>,
 *   jostleNoLongerNeeded: boolean,
 *   requestAnimationFrameActive: boolean,
 *   videoSyncInterval: ReturnType<typeof setTimeout> | undefined,
 * }} Carousel
 */

const /** @type {Carousel[]} */ carousels = [];

function initializeCarousel() {
	const carouselContainers = document.querySelectorAll("[data-carousel]");

	carouselContainers.forEach((carouselContainer) => {
		const slideImages = carouselContainer.querySelectorAll("[data-carousel-slide] [data-carousel-image]");
		const tornLeft = carouselContainer.querySelector("[data-carousel-slide-torn-left]");
		const tornRight = carouselContainer.querySelector("[data-carousel-slide-torn-right]");
		[tornLeft, tornRight].forEach((insertInsideElement) => {
			if (!(insertInsideElement instanceof HTMLElement)) return;
			slideImages.forEach((image) => {
				const clonedImage = image.cloneNode(true);
				if (!(clonedImage instanceof HTMLImageElement) && !(clonedImage instanceof HTMLVideoElement)) return;
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
			videoSyncInterval: undefined,
		};
		carousels.push(carousel);

		images.forEach((image) => {
			if (!(image instanceof HTMLElement)) return;
			image.addEventListener("pointerdown", dragBegin);
		});
		directionPrev?.addEventListener("click", () => slideDirection(carousel, "prev", true, false));
		directionNext?.addEventListener("click", () => slideDirection(carousel, "next", true, false));
		Array.from(dots).forEach((dot) =>
			dot.addEventListener("click", (event) => {
				const index = event.target instanceof Element ? Array.from(dots).indexOf(event.target) : -1;
				slideTo(carousel, index, true);
			}),
		);

		// Jostle hint is a feature to briefly shift the carousel by a bit as a hint to users that it can be interacted with
		if (performJostleHint) {
			window.addEventListener("load", () => {
				if (!(directionPrev instanceof HTMLElement)) return;
				new IntersectionObserver(
					(entries) => {
						entries.forEach((entry) => {
							if (entry.intersectionRatio === 1 && currentTransform(carousel) === 0 && !carousel.jostleNoLongerNeeded) {
								const JOSTLE_TIME = 1000;
								const MAX_JOSTLE_DISTANCE = -10;

								let /** @type {number} */ startTime;
								const buildUp = (/** @type {number} */ timeStep) => {
									if (carousel.jostleNoLongerNeeded) {
										carousel.carouselContainer.classList.remove("jostling");
										return;
									}

									if (!startTime) startTime = timeStep;
									const elapsedTime = timeStep - startTime;

									const easeOutCirc = (/** @type {number} */ x) => Math.sqrt(1 - Math.pow(x - 1, 2));
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
								carousel.carouselContainer.classList.add("jostling");
								requestAnimationFrame(buildUp);
							}
						});
					},
					{ threshold: 1 },
				).observe(directionPrev);
			});
		}
	});
}

/**
 * @param {Carousel} carousel
 * @param {"prev" | "next"} direction
 * @param {boolean} smooth
 */
function slideDirection(carousel, direction, smooth, clamped = false) {
	const directionIndexOffset = { prev: -1, next: 1 }[direction];
	const offsetDotIndex = currentClosestImageIndex(carousel) + directionIndexOffset;

	const nextDotIndex = (offsetDotIndex + carousel.dots.length) % carousel.dots.length;
	const unwrappedNextDotIndex = clamp(offsetDotIndex, 0, carousel.dots.length - 1);

	if (clamped) slideTo(carousel, unwrappedNextDotIndex, smooth);
	else slideTo(carousel, nextDotIndex, smooth);
}

/**
 * @param {Carousel} carousel
 * @param {number} index
 * @param {boolean} smooth
 */
function slideTo(carousel, index, smooth) {
	const activeDot = carousel.carouselContainer.querySelector("[data-carousel-dot].active");
	activeDot?.classList.remove("active");
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

	// Manage video preloading and playback
	manageVideoPlayback(carousel, index);
}

/**
 * Get all video elements for a given slide index (main + torn edge copies)
 * @param {Carousel} carousel
 * @param {number} index
 */
function getVideosForSlide(carousel, index) {
	// Account for the first image being the faded-out last image
	const offsetIndex = index + 1;
	const slideImages = Array.from(carousel.carouselContainer.querySelectorAll("[data-carousel-slide] [data-carousel-image]"));
	const tornLeftImages = Array.from(carousel.carouselContainer.querySelectorAll("[data-carousel-slide-torn-left] [data-carousel-image]"));
	const tornRightImages = Array.from(carousel.carouselContainer.querySelectorAll("[data-carousel-slide-torn-right] [data-carousel-image]"));

	const mainElement = slideImages[offsetIndex];
	const tornLeftElement = tornLeftImages[offsetIndex];
	const tornRightElement = tornRightImages[offsetIndex];

	return {
		main: mainElement instanceof HTMLVideoElement ? mainElement : null,
		tornLeft: tornLeftElement instanceof HTMLVideoElement ? tornLeftElement : null,
		tornRight: tornRightElement instanceof HTMLVideoElement ? tornRightElement : null,
	};
}

/**
 * Check if the carousel is currently in transition (dragging or animating)
 * @param {Carousel} carousel
 */
function isCarouselInTransition(carousel) {
	// Check if user is dragging
	if (carousel.dragLastClientX !== undefined) return true;

	// Check if carousel has the "dragging" class (set during drag)
	if (carousel.carouselContainer.classList.contains("dragging")) return true;

	// Check if any slide has a transition in progress by looking at the computed style
	const firstImage = carousel.images[1];
	if (firstImage instanceof HTMLElement) {
		const style = window.getComputedStyle(firstImage);
		// If transform is transitioning, we're in motion
		if (style.transitionProperty.includes("transform") && style.transitionDuration !== "0s") {
			return true;
		}
	}

	return false;
}

/**
 * Preload and manage playback of videos on current and adjacent slides
 * @param {Carousel} carousel
 * @param {number} currentIndex
 */
function manageVideoPlayback(carousel, currentIndex) {
	const totalSlides = carousel.dots.length;

	// Clear any existing sync interval
	if (carousel.videoSyncInterval !== undefined) {
		clearTimeout(carousel.videoSyncInterval);
		carousel.videoSyncInterval = undefined;
	}

	// Stop all videos that aren't on current or adjacent slides
	for (let i = 0; i < totalSlides; i++) {
		if (Math.abs(i - currentIndex) > 1) {
			const videos = getVideosForSlide(carousel, i);
			[videos.main, videos.tornLeft, videos.tornRight].forEach((video) => {
				if (video) {
					video.pause();
					video.currentTime = 0;
				}
			});
		}
	}

	// Preload and potentially play videos on current and adjacent slides
	const indicesToPreload = [currentIndex - 1, currentIndex, currentIndex + 1].filter((index) => index >= 0 && index < totalSlides);

	indicesToPreload.forEach((index) => {
		const videos = getVideosForSlide(carousel, index);

		// Exit early if not a video slide
		if (!videos.main) return;

		// Preload the video
		if (videos.main.readyState < 3) videos.main.load();

		// If this is the current slide, play the main video when ready
		if (index === currentIndex) {
			const playWhenReady = () => {
				if (videos.main && videos.main.readyState >= 3) {
					// Start the main video
					videos.main.currentTime = 0;
					videos.main.play().catch(() => {});
				} else if (videos.main) {
					// Video not ready yet, check again
					videos.main.addEventListener("canplaythrough", playWhenReady, { once: true });
				}
			};

			playWhenReady();

			// Monitor for transitions and sync torn videos when in motion
			updateVideoSyncForTransitions(carousel, videos);
		}
	});
}

/**
 * Set up monitoring to play/pause torn edge videos based on transition state
 * @param {Carousel} carousel
 * @param {{ main: HTMLVideoElement | null, tornLeft: HTMLVideoElement | null, tornRight: HTMLVideoElement | null }} videos
 */
function updateVideoSyncForTransitions(carousel, videos) {
	if (!videos.main) return;

	const syncTornVideos = () => {
		const inTransition = isCarouselInTransition(carousel);

		// During transition: sync and play all copies
		if (inTransition && videos.main) {
			const mainTime = videos.main.currentTime;
			[videos.tornLeft, videos.tornRight].forEach((video) => {
				if (!video) return;

				if (video.paused) {
					video.currentTime = mainTime;
					video.play().catch(() => {
						// Ignore autoplay errors
					});
				} else {
					// Keep synced
					const drift = Math.abs(video.currentTime - mainTime);
					if (drift > 0.1) {
						video.currentTime = mainTime;
					}
				}
			});
		}
		// Not in transition: pause torn edge videos to save performance
		else {
			if (videos.tornLeft && !videos.tornLeft.paused) videos.tornLeft.pause();
			if (videos.tornRight && !videos.tornRight.paused) videos.tornRight.pause();
		}

		// Continue checking while in transition, or check again soon in case transition starts
		carousel.videoSyncInterval = setTimeout(syncTornVideos, 100);
	};

	syncTornVideos();
}

/**
 * @param {Carousel} carousel
 */
function currentTransform(carousel) {
	const currentTransformMatrix = window.getComputedStyle(carousel.images[1]).transform;
	// Grab the X value from the format that looks like either of: `matrix(1, 0, 0, 1, -1332.13, 0)` or `none`
	const xValue = Number(currentTransformMatrix.split(",")[4] || "0");

	return xValue + carousel.images[1].getBoundingClientRect().width;
}

/**
 * @param {Carousel} carousel
 * @param {number} x
 * @param {string} unit
 * @param {boolean} smooth
 * @param {boolean} doNotTerminateJostle
 */
function setCurrentTransform(carousel, x, unit, smooth, doNotTerminateJostle = false) {
	const xInitial = currentTransform(carousel);
	let xValue = x;
	if (unit === "%") xValue = x - 100;
	if (unit === "px") xValue = x - carousel.images[1].getBoundingClientRect().width;

	Array.from(carousel.images).forEach((image) => {
		if (!(image instanceof HTMLElement)) return;
		image.style.transitionTimingFunction = smooth ? "ease-in-out" : "cubic-bezier(0, 0, 0.2, 1)";
		image.style.transform = `translateX(${xValue}${unit})`;
	});

	// If the user caused the carousel to move, we can assume they know how to use it and don't need the jostle hint anymore
	if (!doNotTerminateJostle && x - xInitial < 0.0001) carousel.jostleNoLongerNeeded = true;

	const distance = unit === "%" ? x : (x / carousel.images[1].getBoundingClientRect().width) * 100;
	const overSlidingLeft = distance > 0;
	const overSlidingRight = distance < (carousel.dots.length - 1) * -100;

	if ((overSlidingLeft || overSlidingRight) && !carousel.requestAnimationFrameActive) updateOverSlide(carousel);
}

/**
 * @param {Carousel} carousel
 */
function updateOverSlide(carousel) {
	const paddingLeft = parseInt(getComputedStyle(carousel.images[1]).paddingLeft);
	const paddingRight = parseInt(getComputedStyle(carousel.images[carousel.images.length - 2]).paddingRight);
	const slidLeftDistance = carousel.images[1].getBoundingClientRect().left + paddingLeft - (carousel.images[1].parentElement?.getBoundingClientRect().left || 0);
	const slidRightDistance = -(carousel.images[carousel.images.length - 2].getBoundingClientRect().right - paddingRight - (carousel.images[1].parentElement?.getBoundingClientRect().right || 0));
	const imageWidth = carousel.images[1].getBoundingClientRect().width;
	const overSlideFactor = Math.min(1, Math.max(0, Math.max(slidLeftDistance, slidRightDistance) / imageWidth));

	const images = carousel.images[0].closest("[data-carousel]")?.querySelectorAll("[data-carousel-image]:first-child, [data-carousel-image]:last-child");
	if (!images) return;

	// Call again the next frame if we're still sliding past the edge
	if (overSlideFactor > 0) {
		images.forEach((image) => {
			if (!(image instanceof HTMLElement)) return;
			image.style.setProperty("--over-slide-factor", `${overSlideFactor}`);
		});

		carousel.requestAnimationFrameActive = true;
		requestAnimationFrame(() => updateOverSlide(carousel));
	} else {
		images.forEach((image) => {
			if (!(image instanceof HTMLElement)) return;
			image.style.removeProperty("--over-slide-factor");
		});

		carousel.requestAnimationFrameActive = false;
	}
}

/**
 * @param {Carousel} carousel
 */
function currentClosestImageIndex(carousel) {
	const currentTransformX = -currentTransform(carousel);

	const imageWidth = carousel.images[1].getBoundingClientRect().width;
	return Math.round(currentTransformX / imageWidth);
}

/**
 * @param {Carousel} carousel
 */
function currentActiveDotIndex(carousel) {
	const activeDot = carousel.carouselContainer.querySelector("[data-carousel-dot].active");
	return activeDot ? Array.from(carousel.dots).indexOf(activeDot) : -1;
}

/**
 * @param {PointerEvent} event
 */
function dragBegin(event) {
	if (!(event.target instanceof HTMLElement)) return;
	const carouselContainer = event.target.closest("[data-carousel]");
	const carousel = carousels.find((carousel) => carousel.carouselContainer === carouselContainer);
	if (!carousel) return;

	event.preventDefault();

	carousel.dragLastClientX = event.clientX;

	setCurrentTransform(carousel, currentTransform(carousel), "px", false);
	carouselContainer?.classList.add("dragging");
}

/**
 * @param {boolean} dropWithoutVelocity
 */
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

/**
 * @param {PointerEvent} event
 */
function dragMove(event) {
	if (!(event.target instanceof HTMLElement)) return;

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

/**
 * @param {number} value
 * @param {number} min
 * @param {number} max
 */
function clamp(value, min, max) {
	const m = Math; // This is a workaround for a bug in Zola's minifier
	return m.min(m.max(value, min), max);
}

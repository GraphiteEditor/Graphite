const NAV_BUTTON_INITIAL_FONT_SIZE = 28; // Keep up to date with the initial `--nav-font-size` in base.scss
const RIPPLE_ANIMATION_MILLISECONDS = 100;
const RIPPLE_WIDTH = 100;
const HANDLE_STRETCH = 0.4;

let navButtons;
let rippleSvg;
let ripplePath;
let fullRippleHeight;
let ripples;
let activeRippleIndex;

window.addEventListener("DOMContentLoaded", initializeRipples);

function initializeRipples() {
	window.addEventListener("resize", () => animate(true));

	navButtons = document.querySelectorAll("header nav a");
	rippleSvg = document.querySelector("header .ripple");
	ripplePath = rippleSvg.querySelector("path");
	fullRippleHeight = Number.parseInt(window.getComputedStyle(rippleSvg).height, 10);

	ripples = Array.from(navButtons).map((button) => ({
		element: button,
		goingUp: false,
		animationStartTime: 0,
		animationEndTime: 0,
	}));

	activeRippleIndex = ripples.findIndex((ripple) => {
		let link = ripple.element.getAttribute("href");
		if (!link.endsWith("/")) link += "/";
		let location = window.location.pathname;
		if (!location.endsWith("/")) location += "/";

		// Special case for the root, which will otherwise match as the starting prefix of all pages
		if (link === "/" && location === "/") return true;
		if (link === "/") return false;

		return location.startsWith(link);
	});

	ripples.forEach((ripple) => {
		const updateTimings = (goingUp) => {
			const start = ripple.animationStartTime;
			const now = Date.now();
			const stop = ripple.animationStartTime + RIPPLE_ANIMATION_MILLISECONDS;

			const elapsed = now - start;
			const remaining = stop - now;

			ripple.goingUp = goingUp;
			// Encode the potential reversing of direction via the animation start and end times
			ripple.animationStartTime = now < stop ? now - remaining : now;
			ripple.animationEndTime = now < stop ? now + elapsed : now + RIPPLE_ANIMATION_MILLISECONDS;

			animate();
		};

		ripple.element.addEventListener("pointerenter", () => updateTimings(true));
		ripple.element.addEventListener("pointerleave", () => updateTimings(false));
	});

	if (activeRippleIndex >= 0) ripples[activeRippleIndex] = {
		...ripples[activeRippleIndex],
		goingUp: true,
		// Set to non-zero, but very old times (1ms after epoch), so the math works out as if the animation has already completed
		animationStartTime: 1,
		animationEndTime: 1 + RIPPLE_ANIMATION_MILLISECONDS,
	};

	setRipples();
}

function animate(forceRefresh = false) {
	const FUZZ_MILLISECONDS = 100;
	const animateThisFrame = ripples.some((ripple) => ripple.animationStartTime > 0 && ripple.animationEndTime > 0 && Date.now() <= ripple.animationEndTime + FUZZ_MILLISECONDS);

	if (animateThisFrame || forceRefresh) {
		setRipples();
		window.requestAnimationFrame(() => animate());
	}
}

function setRipples() {
	const lerp = (a, b, t) => a + (b - a) * t;
	const ease = (x) => 1 - (1 - x) * (1 - x);
	const clamp01 = (x) => Math.min(Math.max(x, 0), 1);
	
	const rippleSvgRect = rippleSvg.getBoundingClientRect();

	const rippleStrokeWidth = Number.parseInt(window.getComputedStyle(ripplePath).getPropertyValue("--border-thickness"), 10);
	const navButtonFontSize = Number.parseInt(window.getComputedStyle(navButtons[0]).fontSize, 10) || NAV_BUTTON_INITIAL_FONT_SIZE;
	const mediaQueryScaleFactor = navButtonFontSize / NAV_BUTTON_INITIAL_FONT_SIZE;

	// Position of bottom centerline to top centerline
	const rippleBaselineCenterline = fullRippleHeight - rippleStrokeWidth / 2;
	const rippleToplineCenterline = rippleStrokeWidth / 2;

	let path = `M -16,${rippleBaselineCenterline - 16} L 0,${rippleBaselineCenterline} `;

	ripples.forEach((ripple) => {
		if (ripple.animationStartTime === 0 || ripple.animationEndTime === 0) return;

		const elapsed = Date.now() - ripple.animationStartTime;
		const duration = ripple.animationEndTime - ripple.animationStartTime;
		const t = ease(clamp01(elapsed / duration));

		const bumpCrestRaiseFactor = (ripple.goingUp ? t : 1 - t) * mediaQueryScaleFactor;
		const bumpCrest = lerp(rippleToplineCenterline, rippleBaselineCenterline, bumpCrestRaiseFactor);
		const bumpCrestDelta = bumpCrest - rippleStrokeWidth / 2;

		const buttonRect = ripple.element.getBoundingClientRect();
		const buttonCenter = buttonRect.width / 2;
		const rippleCenter = RIPPLE_WIDTH / 2 * mediaQueryScaleFactor;
		const rippleOffset = rippleCenter - buttonCenter;
		const rippleStartX = buttonRect.left - rippleSvgRect.left - rippleOffset;
		const handleRadius = rippleCenter * HANDLE_STRETCH;

		path += `L ${rippleStartX},${rippleBaselineCenterline} `;
		path += `c ${handleRadius},0 ${rippleCenter - handleRadius},${-bumpCrestDelta} ${rippleCenter},${-bumpCrestDelta} `;
		path += `s ${rippleCenter - handleRadius},${bumpCrestDelta} ${rippleCenter},${bumpCrestDelta} `;
	});

	path += `L ${rippleSvgRect.width + 16},${rippleBaselineCenterline} L${rippleSvgRect.width + 16},${rippleBaselineCenterline - 16}`;

	ripplePath.setAttribute("d", path);
}

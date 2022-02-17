const NAV_BUTTON_INITIAL_FONT_SIZE = 36;
const RIPPLE_ANIMATION_MILLISECONDS = 100;
const RIPPLE_WIDTH = 150;
const HANDLE_STRETCH = 0.4;

let ripplesInitialized;
let navButtons;
let navButtonFontSize;
let rippleSvg;
let ripplePath;
let fullRippleHeight;
let ripples;
let activeRippleIndex;

window.addEventListener("DOMContentLoaded", initializeRipples);
window.addEventListener("resize", () => animate(true));

function setRipples(mediaQueryScaleFactor) {
	const rippleSvgRect = rippleSvg.getBoundingClientRect();
	const rippleSvgLeft = rippleSvgRect.left;
	const rippleSvgWidth = rippleSvgRect.width;

	let path = `M 0,${fullRippleHeight + 3} `;

	ripples.forEach((ripple) => {
		if (!ripple.animationStartTime || !ripple.animationEndTime) return;

		const t = Math.min((Date.now() - ripple.animationStartTime) / (ripple.animationEndTime - ripple.animationStartTime), 1);
		const height = fullRippleHeight * (ripple.goingUp ? ease(t) : 1 - ease(t));

		const buttonRect = ripple.element.getBoundingClientRect();

		const buttonCenter = buttonRect.width / 2;
		const rippleCenter = RIPPLE_WIDTH / 2 * mediaQueryScaleFactor;
		const rippleOffset = rippleCenter - buttonCenter;

		const rippleStartX = buttonRect.left - rippleSvgLeft - rippleOffset;

		const rippleRadius = RIPPLE_WIDTH / 2 * mediaQueryScaleFactor;
		const handleRadius = rippleRadius * HANDLE_STRETCH;

		path += `L ${rippleStartX},${fullRippleHeight + 3} `;
		path += `c ${handleRadius},0 ${rippleRadius - handleRadius},-${height} ${rippleRadius},-${height} `;
		path += `s ${rippleRadius - handleRadius},${height} ${rippleRadius},${height} `;
	});

	path += `l ${rippleSvgWidth},0`;

	ripplePath.setAttribute("d", path);
}

function initializeRipples() {
	ripplesInitialized = true;

	navButtons = document.querySelectorAll("header nav a");
	rippleSvg = document.querySelector("header .ripple");
	ripplePath = rippleSvg.querySelector("path");
	fullRippleHeight = Number.parseInt(window.getComputedStyle(rippleSvg).height) - 4;

	ripples = Array.from(navButtons).map((button) => ({
		element: button,
		animationStartTime: null,
		animationEndTime: null,
		goingUp: false,
	}));

	activeRippleIndex = ripples.findIndex((ripple) => ripple.element.getAttribute("href") === window.location.pathname);


	ripples.forEach((ripple) => {
		const updateTimings = (goingUp) => {
			const start = ripple.animationStartTime;
			const now = Date.now();
			const stop = ripple.animationStartTime + RIPPLE_ANIMATION_MILLISECONDS;

			const elapsed = now - start;
			const remaining = stop - now;

			ripple.animationStartTime = now < stop ? now - remaining : now;
			ripple.animationEndTime = now < stop ? now + elapsed : now + RIPPLE_ANIMATION_MILLISECONDS;

			ripple.goingUp = goingUp;
			animate();
		};

		ripple.element.addEventListener("pointerenter", () => updateTimings(true));
		ripple.element.addEventListener("pointerleave", () => updateTimings(false));
	});

	ripples[activeRippleIndex] = {
		...ripples[activeRippleIndex],
		animationStartTime: Date.now(),
		animationEndTime: Date.now() + RIPPLE_ANIMATION_MILLISECONDS,
		goingUp: true,
	};

	animate();
}

function animate(forceRefresh) {
	if (!ripplesInitialized) return;

	navButtonFontSize = Number.parseInt(window.getComputedStyle(navButtons[0]).fontSize) || 36;
	const mediaQueryScaleFactor = navButtonFontSize / NAV_BUTTON_INITIAL_FONT_SIZE;

	const animateThisFrame = ripples.some((ripple) => ripple.animationStartTime && ripple.animationEndTime && Date.now() <= ripple.animationEndTime);
	if (animateThisFrame || forceRefresh) {
		setRipples(mediaQueryScaleFactor);
		window.requestAnimationFrame(animate);
	}
}

function ease(x) {
	return 1 - (1 - x) * (1 - x);
}

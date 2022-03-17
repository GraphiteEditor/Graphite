const NAV_BUTTON_INITIAL_FONT_SIZE = 32;
const RIPPLE_ANIMATION_MILLISECONDS = 100;
const RIPPLE_WIDTH = 140;
const HANDLE_STRETCH = 0.4;

let ripplesInitialized;
let navButtons;
let rippleSvg;
let ripplePath;
let fullRippleHeight;
let ripples;
let activeRippleIndex;

let globalCount = 0;

window.addEventListener("DOMContentLoaded", initializeRipples);
window.addEventListener("resize", () => animate(true));

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

	activeRippleIndex = ripples.findIndex((ripple) => ripple.element.getAttribute("href").replace(/\//g, "") === window.location.pathname.replace(/\//g, ""));


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
			animate(false);
		};

		ripple.element.addEventListener("pointerenter", () => updateTimings(true));
		ripple.element.addEventListener("pointerleave", () => updateTimings(false));
	});

	ripples[activeRippleIndex] = {
		...ripples[activeRippleIndex],
		animationStartTime: 1,
		animationEndTime: 1 + RIPPLE_ANIMATION_MILLISECONDS,
		goingUp: true,
	};

	setRipples();
}

function animate(forceRefresh) {
	if (!ripplesInitialized) return;
	
	const animateThisFrame = ripples.some((ripple) => ripple.animationStartTime && ripple.animationEndTime && Date.now() <= ripple.animationEndTime);

	console.log(globalCount, new Date().getSeconds(), Date.now(), animateThisFrame, {...ripples[0]});
	globalCount++;

	if (animateThisFrame || forceRefresh) {
		setRipples();
		window.requestAnimationFrame(() => animate(false));
	}
}

function setRipples() {
	const navButtonFontSize = Number.parseInt(window.getComputedStyle(navButtons[0]).fontSize) || NAV_BUTTON_INITIAL_FONT_SIZE;
	const mediaQueryScaleFactor = navButtonFontSize / NAV_BUTTON_INITIAL_FONT_SIZE;

	const rippleHeight = fullRippleHeight * (mediaQueryScaleFactor * 0.5 + 0.5);
	const rippleSvgRect = rippleSvg.getBoundingClientRect();
	const rippleSvgLeft = rippleSvgRect.left;
	const rippleSvgWidth = rippleSvgRect.width;

	let path = `M 0,${rippleHeight + 3} `;

	ripples.forEach((ripple) => {
		if (!ripple.animationStartTime || !ripple.animationEndTime) return;

		const t = Math.min((Date.now() - ripple.animationStartTime) / (ripple.animationEndTime - ripple.animationStartTime), 1);
		const height = rippleHeight * (ripple.goingUp ? ease(t) : 1 - ease(t));

		const buttonRect = ripple.element.getBoundingClientRect();

		const buttonCenter = buttonRect.width / 2;
		const rippleCenter = RIPPLE_WIDTH / 2 * mediaQueryScaleFactor;
		const rippleOffset = rippleCenter - buttonCenter;

		const rippleStartX = buttonRect.left - rippleSvgLeft - rippleOffset;

		const rippleRadius = RIPPLE_WIDTH / 2 * mediaQueryScaleFactor;
		const handleRadius = rippleRadius * HANDLE_STRETCH;

		path += `L ${rippleStartX},${rippleHeight + 3} `;
		path += `c ${handleRadius},0 ${rippleRadius - handleRadius},${-height} ${rippleRadius},${-height} `;
		path += `s ${rippleRadius - handleRadius},${height} ${rippleRadius},${height} `;
	});

	path += `l ${rippleSvgWidth},0`;

	ripplePath.setAttribute("d", path);
}

function ease(x) {
	return 1 - (1 - x) * (1 - x);
}

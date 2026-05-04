// Keep up to date with the initial `--nav-font-size` in base.scss
const NAV_BUTTON_INITIAL_FONT_SIZE = 28;

// Local "lift" bump under each hovered/active button (gravitational attractor that pulls the surface up)
const BUMP_RAISE_MILLISECONDS = 120;
const BUMP_WIDTH = 100;

// Propagating wave pulse emitted when a lifted button drops back down (the splash from removing your finger from the water)
const WAVE_SPEED_PX_PER_SECOND = 1000;
const WAVE_PACKET_SIGMA = 200;
const WAVE_WAVELENGTH = 300;
const WAVE_AMPLITUDE = 10;
const WAVE_ATTENUATION_LENGTH = 500;
const WAVE_RAMP_UP_MILLISECONDS = 80;
const WAVE_PRUNE_AMPLITUDE = 0.15;
const WAVE_SAMPLE_SPACING = 6;

// Wider-than-the-bump zone around each lifted button where a passing wave's contribution to the surface is locally damped, so the bump doesn't tilt or jiggle when waves pass through it
const WAVE_SUPPRESSION_HALF_WIDTH = 200;

let /** @type {NodeList | undefined} **/ navButtons;
let /** @type {Element | undefined} **/ rippleSvg;
let /** @type {Element | undefined} **/ rippleMaskPath;
let /** @type {Element | undefined} **/ rippleLinePath;
let /** @type {Element | undefined} **/ rippleTaperLeft;
let /** @type {Element | undefined} **/ rippleTaperRight;
let /** @type {number | undefined} **/ baselineFromTop;
let /** @type {number | undefined} **/ taperHalfWidth;
let /** @type {{ element: HTMLElement, goingUp: boolean, animationStartTime: number, animationEndTime: number }[]} **/ ripples;
let /** @type {number} **/ activeRippleIndex;
let /** @type {{ originX: number, startTime: number }[]} **/ wavePulses = [];

window.addEventListener("DOMContentLoaded", initializeRipples);

function initializeRipples() {
	window.addEventListener("resize", () => animate(true));

	navButtons = document.querySelectorAll("header nav a") || undefined;
	rippleSvg = document.querySelector("header .ripple") || undefined;
	rippleMaskPath = rippleSvg?.querySelector(".ripple-mask") || undefined;
	rippleLinePath = rippleSvg?.querySelector(".ripple-line") || undefined;
	rippleTaperLeft = rippleSvg?.querySelector(".ripple-taper-left") || undefined;
	rippleTaperRight = rippleSvg?.querySelector(".ripple-taper-right") || undefined;
	baselineFromTop = rippleSvg ? Number.parseInt(window.getComputedStyle(rippleSvg).getPropertyValue("--ripple-baseline-from-top"), 10) || undefined : undefined;
	taperHalfWidth = rippleSvg ? Number.parseInt(window.getComputedStyle(rippleSvg).getPropertyValue("--ripple-taper-half-width"), 10) || undefined : undefined;

	ripples = Array.from(navButtons)
		.filter((x) => x instanceof HTMLElement)
		.map((button) => ({
			element: button,
			goingUp: false,
			animationStartTime: 0,
			animationEndTime: 0,
		}));

	activeRippleIndex = ripples.findIndex((ripple) => {
		let link = ripple.element.getAttribute("href");
		if (!link) return false;
		if (!link.endsWith("/")) link += "/";
		let location = window.location.pathname;
		if (!location.endsWith("/")) location += "/";

		// Special case for the root, which will otherwise match as the starting prefix of all pages
		if (link === "/" && location === "/") return true;
		if (link === "/") return false;

		return location.startsWith(link);
	});

	ripples.forEach((ripple) => {
		const updateTimings = (/** @type {boolean} **/ goingUp) => {
			const start = ripple.animationStartTime;
			const now = Date.now();
			const stop = ripple.animationStartTime + BUMP_RAISE_MILLISECONDS;

			const elapsed = now - start;
			const remaining = stop - now;

			ripple.goingUp = goingUp;
			// Encode the potential reversing of direction via the animation start and end times
			ripple.animationStartTime = now < stop ? now - remaining : now;
			ripple.animationEndTime = now < stop ? now + elapsed : now + BUMP_RAISE_MILLISECONDS;

			// Only the drop emits a ripple, like releasing a finger from the water surface — the lift only deforms it locally
			if (!goingUp) emitWavePulse(ripple);
			animate();
		};

		ripple.element.addEventListener("pointerenter", () => updateTimings(true));
		ripple.element.addEventListener("pointerleave", () => updateTimings(false));
	});

	if (activeRippleIndex >= 0) {
		ripples[activeRippleIndex] = {
			...ripples[activeRippleIndex],
			goingUp: true,
			// Set to non-zero, but very old times (1ms after epoch), so the math works out as if the animation has already completed
			animationStartTime: 1,
			animationEndTime: 1 + BUMP_RAISE_MILLISECONDS,
		};
	}

	setRipples();
}

function emitWavePulse(/** @type {{ element: HTMLElement }} **/ ripple) {
	if (!rippleSvg) return;

	const buttonRect = ripple.element.getBoundingClientRect();
	const svgRect = rippleSvg.getBoundingClientRect();
	const originX = buttonRect.left - svgRect.left + buttonRect.width / 2;

	wavePulses.push({
		originX,
		startTime: Date.now(),
	});
}

function animate(forceRefresh = false) {
	const now = Date.now();

	// Drop pulses whose amplitude has decayed below the visible threshold
	wavePulses = wavePulses.filter((pulse) => {
		const traveled = (WAVE_SPEED_PX_PER_SECOND * (now - pulse.startTime)) / 1000;
		return Math.exp(-traveled / WAVE_ATTENUATION_LENGTH) > WAVE_PRUNE_AMPLITUDE / WAVE_AMPLITUDE;
	});

	const FUZZ_MILLISECONDS = 100;
	const bumpsAnimating = ripples.some((ripple) => ripple.animationStartTime > 0 && ripple.animationEndTime > 0 && now <= ripple.animationEndTime + FUZZ_MILLISECONDS);
	const wavesActive = wavePulses.length > 0;

	if (bumpsAnimating || wavesActive || forceRefresh) {
		setRipples();
		window.requestAnimationFrame(() => animate());
	}
}

function setRipples() {
	const ease = (/** @type {number} **/ x) => 1 - (1 - x) * (1 - x);
	const clamp01 = (/** @type {number} **/ x) => Math.min(Math.max(x, 0), 1);

	if (!rippleSvg || !rippleMaskPath || !rippleLinePath) return;
	if (!rippleTaperLeft || !rippleTaperRight) return;
	if (!navButtons || !baselineFromTop || !taperHalfWidth) return;
	if (!(navButtons[0] instanceof HTMLElement)) return;

	const now = Date.now();
	const rippleSvgRect = rippleSvg.getBoundingClientRect();

	const rippleStrokeWidth = Number.parseInt(window.getComputedStyle(rippleSvg).getPropertyValue("--border-thickness"), 10);
	const navButtonFontSize = Number.parseInt(window.getComputedStyle(navButtons[0]).fontSize, 10) || NAV_BUTTON_INITIAL_FONT_SIZE;
	const mediaQueryScaleFactor = navButtonFontSize / NAV_BUTTON_INITIAL_FONT_SIZE;

	// Baseline centerline: --ripple-baseline-from-top marks where the bottom edge of the baseline stroke sits, so the centerline is half a stroke above
	const baselineY = baselineFromTop - rippleStrokeWidth / 2;
	const toplineY = rippleStrokeWidth / 2;
	const maxBumpHeight = baselineY - toplineY;

	// Snapshot per-button lift state for this frame: a "gravity" bump that pulls the surface up linearly
	const bumpHalfWidth = (BUMP_WIDTH / 2) * mediaQueryScaleFactor;
	const suppressionHalfWidth = WAVE_SUPPRESSION_HALF_WIDTH * mediaQueryScaleFactor;
	const bumps = ripples
		.map((ripple) => {
			if (ripple.animationStartTime === 0 && ripple.animationEndTime === 0) return null;

			const elapsed = now - ripple.animationStartTime;
			const duration = ripple.animationEndTime - ripple.animationStartTime;
			const t = ease(clamp01(elapsed / duration));
			const liftFraction = clamp01(ripple.goingUp ? t : 1 - t);
			if (liftFraction <= 0) return null;

			const buttonRect = ripple.element.getBoundingClientRect();
			const centerX = buttonRect.left - rippleSvgRect.left + buttonRect.width / 2;

			return { centerX, height: maxBumpHeight * liftFraction * mediaQueryScaleFactor, halfWidth: bumpHalfWidth, liftFraction, suppressionHalfWidth };
		})
		.filter((bump) => bump !== null);

	// Snapshot per-pulse propagation state for this frame
	const pulses = wavePulses.map((pulse) => {
		const ageMs = now - pulse.startTime;
		const ageSeconds = ageMs / 1000;
		const traveled = WAVE_SPEED_PX_PER_SECOND * ageSeconds;
		const rampFactor = clamp01(ageMs / WAVE_RAMP_UP_MILLISECONDS);
		const distanceAttenuation = Math.exp(-traveled / WAVE_ATTENUATION_LENGTH);
		const sigma = WAVE_PACKET_SIGMA * mediaQueryScaleFactor;
		const wavelength = WAVE_WAVELENGTH * mediaQueryScaleFactor;
		const amplitude = WAVE_AMPLITUDE * mediaQueryScaleFactor * rampFactor * distanceAttenuation;
		return { originX: pulse.originX, traveled, sigma, wavelength, amplitude };
	});

	// Sample the surface: the lift bump adds directly while the wave is damped within a vicinity around each lifted button to avoid jiggling the bump
	const sampleSpacing = WAVE_SAMPLE_SPACING * mediaQueryScaleFactor;
	const numSamples = Math.max(2, Math.ceil(rippleSvgRect.width / sampleSpacing) + 1);
	const samples = new Array(numSamples);

	for (let i = 0; i < numSamples; i++) {
		const x = (i / (numSamples - 1)) * rippleSvgRect.width;

		let liftHeight = 0;
		let waveSuppression = 0;
		for (const bump of bumps) {
			const dist = x - bump.centerX;

			if (Math.abs(dist) < bump.halfWidth) {
				const shape = Math.cos((Math.PI * dist) / (2 * bump.halfWidth)) ** 2;
				liftHeight += bump.height * shape;
			}

			// Wave damping zone: a wider cos² envelope around each lifted button, scaled by how lifted that button currently is
			if (Math.abs(dist) < bump.suppressionHalfWidth) {
				const shape = Math.cos((Math.PI * dist) / (2 * bump.suppressionHalfWidth)) ** 2;
				waveSuppression += bump.liftFraction * shape;
			}
		}
		waveSuppression = Math.min(1, waveSuppression);

		let waveHeight = 0;
		for (const pulse of pulses) {
			// d'Alembert split: the source disturbance radiates as two equal halves moving in opposite directions
			for (const direction of [-1, 1]) {
				const center = pulse.originX + direction * pulse.traveled;
				const dist = x - center;
				const distNorm = dist / pulse.sigma;
				if (Math.abs(distNorm) > 4) continue;
				const envelope = Math.exp(-distNorm * distNorm);
				const oscillation = Math.cos((2 * Math.PI * dist) / pulse.wavelength);
				waveHeight += 0.5 * pulse.amplitude * envelope * oscillation;
			}
		}

		const displacement = liftHeight + waveHeight * (1 - waveSuppression);
		samples[i] = { x, y: baselineY - displacement };
	}

	const waveCurve = buildSmoothCurve(samples);
	const cornerY = baselineY - 16;
	const leftCornerX = -16;
	const rightCornerX = rippleSvgRect.width + 16;
	const last = samples[samples.length - 1];

	// Mask: closed region above the wave that hides navbar content under the SVG. Includes off-screen corners for a clean fill closure.
	const maskPath = `M ${leftCornerX},${cornerY} L ${samples[0].x.toFixed(2)},${samples[0].y.toFixed(2)} ${waveCurve} L ${rightCornerX},${last.y.toFixed(2)} L ${rightCornerX},${cornerY}`;
	rippleMaskPath.setAttribute("d", maskPath);

	// Visible wave line: just the curve, no off-screen extensions, so its stroke never appears outside the SVG bounds
	const linePath = `M ${samples[0].x.toFixed(2)},${samples[0].y.toFixed(2)} ${waveCurve}`;
	rippleLinePath.setAttribute("d", linePath);

	// Tapered end caps: apex sits at the baseline's bottom edge so the bottom stays flat while the top slopes down to meet it, matching the original CSS-border triangles
	const halfStroke = rippleStrokeWidth / 2;
	const apexY = baselineY + halfStroke;
	const leftApexX = -taperHalfWidth;
	const rightApexX = rippleSvgRect.width + taperHalfWidth;
	const wideRightX = rippleSvgRect.width.toFixed(2);
	const leftPoints = `${leftApexX},${apexY} 0,${(samples[0].y - halfStroke).toFixed(2)} 0,${(samples[0].y + halfStroke).toFixed(2)}`;
	const rightPoints = `${rightApexX},${apexY} ${wideRightX},${(last.y - halfStroke).toFixed(2)} ${wideRightX},${(last.y + halfStroke).toFixed(2)}`;
	rippleTaperLeft.setAttribute("points", leftPoints);
	rippleTaperRight.setAttribute("points", rightPoints);
}

function buildSmoothCurve(/** @type {{ x: number, y: number }[]} **/ samples) {
	const get = (/** @type {number} **/ index) => {
		if (index < 0) {
			// Reflect first segment to derive a virtual point with matching tangent
			const a = samples[0];
			const b = samples[1];
			return { x: 2 * a.x - b.x, y: 2 * a.y - b.y };
		}
		if (index >= samples.length) {
			const a = samples[samples.length - 1];
			const b = samples[samples.length - 2];
			return { x: 2 * a.x - b.x, y: 2 * a.y - b.y };
		}
		return samples[index];
	};

	// Catmull-Rom-to-cubic-Bezier across the sample chain for a smooth surface curve
	let curve = "";
	for (let i = 0; i < samples.length - 1; i++) {
		const p0 = get(i - 1);
		const p1 = samples[i];
		const p2 = samples[i + 1];
		const p3 = get(i + 2);

		const cp1x = p1.x + (p2.x - p0.x) / 6;
		const cp1y = p1.y + (p2.y - p0.y) / 6;
		const cp2x = p2.x - (p3.x - p1.x) / 6;
		const cp2y = p2.y - (p3.y - p1.y) / 6;

		curve += `C ${cp1x.toFixed(2)},${cp1y.toFixed(2)} ${cp2x.toFixed(2)},${cp2y.toFixed(2)} ${p2.x.toFixed(2)},${p2.y.toFixed(2)} `;
	}

	return curve;
}

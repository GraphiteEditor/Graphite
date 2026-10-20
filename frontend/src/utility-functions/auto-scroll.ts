// How far inside a container's top or bottom edge a drag starts scrolling it
const EDGE_ZONE_PIXELS = 25;
// How far past the edge the pointer keeps speeding up the scroll, as with the canvas's auto-panning
const MAX_OVEREXTENSION_PIXELS = 50;
// Scroll speed in pixels per second for each pixel the pointer reaches into the edge zone
const SPEED_PER_PIXEL_OF_REACH = 10;
// How far the sides of the triangle toward a drop target bow outward at their middle, as a fraction of their length
const AIM_BOW_FRACTION = 0.1;
// How far outside that triangle the scroll speed eases back up to full
const AIM_SOFTNESS_PIXELS = 12;

export type AutoScroller = {
	// Given on each move of a drag, with the container it's in, the pointer's position in the viewport, and the fraction of the usual speed to scroll at
	update: (container: HTMLElement, clientX: number, clientY: number, speed: number) => void;
	stop: () => void;
};

// The fraction of its usual speed auto-scrolling keeps while a drag begun at `start` may be headed for the `target` area to drop there instead.
// It's none within the triangle from the start to the target (sides bowed outward, cut off at the target's bottom), easing up to all of it outside.
export function speedWhileAimingAt(start: [number, number], clientX: number, clientY: number, target: DOMRect): number {
	const corners: [number, number][] = [
		[target.left, target.top],
		[target.right, target.top],
		[target.left, target.bottom],
		[target.right, target.bottom],
	];

	// Angles around the start, measured from the direction of the target's center so none wrap around past a half turn
	const towardCenter = Math.atan2((target.top + target.bottom) / 2 - start[1], (target.left + target.right) / 2 - start[0]);
	const angleFromStart = (x: number, y: number) => {
		const angle = Math.atan2(y - start[1], x - start[0]) - towardCenter;
		return Math.atan2(Math.sin(angle), Math.cos(angle));
	};
	const cornerAngles = corners.map(([x, y]) => angleFromStart(x, y));
	const pointerAngle = angleFromStart(clientX, clientY);

	// The triangle's side nearest the pointer, and how far the pointer's angle is outside it (negative when inside the triangle)
	const minAngle = Math.min(...cornerAngles);
	const maxAngle = Math.max(...cornerAngles);
	const sideAngle = pointerAngle - maxAngle > minAngle - pointerAngle ? maxAngle : minAngle;
	const sideCorner = corners[cornerAngles.indexOf(sideAngle)];
	const angleOutside = Math.max(minAngle - pointerAngle, pointerAngle - maxAngle);

	// The side bows outward along a parabolic arc from the start to its corner on the target
	const pointerDistance = Math.hypot(clientX - start[0], clientY - start[1]);
	const sideLength = Math.hypot(sideCorner[0] - start[0], sideCorner[1] - start[1]);
	const alongSide = (pointerDistance * Math.cos(angleOutside)) / sideLength;
	const bow = alongSide > 0 && alongSide < 1 ? 4 * AIM_BOW_FRACTION * sideLength * alongSide * (1 - alongSide) : 0;
	const distanceOutside = pointerDistance * Math.sin(Math.min(angleOutside, Math.PI / 2)) - bow;

	const besideAim = smoothstep(0, AIM_SOFTNESS_PIXELS, distanceOutside);
	const beyondTarget = smoothstep(target.bottom, target.bottom + AIM_SOFTNESS_PIXELS, clientY);
	return Math.max(besideAim, beyondTarget);
}

function smoothstep(from: number, to: number, value: number): number {
	const t = Math.min(Math.max((value - from) / (to - from), 0), 1);
	return t * t * (3 - 2 * t);
}

// Scrolls a container while a drag holds the pointer near or past its top or bottom edge (but not off to its side), faster the further it reaches.
// After each scroll, `onScroll` lets the drag update whatever depends on the content now beneath the still pointer.
export function createAutoScroller(onScroll: () => void): AutoScroller {
	let container: HTMLElement | undefined;
	let clientX = 0;
	let clientY = 0;
	let speed = 1;
	let animationFrame: number | undefined;
	let previousTimestamp: number | undefined;
	// The fraction of a pixel not yet scrolled, kept since a container may only scroll by whole pixels
	let unscrolled = 0;

	const stopLoop = () => {
		if (animationFrame !== undefined) cancelAnimationFrame(animationFrame);
		animationFrame = undefined;
		previousTimestamp = undefined;
		unscrolled = 0;
	};

	const frame = (timestamp: number) => {
		if (!container) {
			stopLoop();
			return;
		}

		const rect = container.getBoundingClientRect();
		const reachIntoTop = rect.top + EDGE_ZONE_PIXELS - clientY;
		const reachIntoBottom = clientY - (rect.bottom - EDGE_ZONE_PIXELS);
		const maxReach = EDGE_ZONE_PIXELS + MAX_OVEREXTENSION_PIXELS;
		const besideContainer = clientX >= rect.left && clientX <= rect.right;
		let velocity = 0;
		if (besideContainer && reachIntoTop > 0) velocity = -Math.min(reachIntoTop, maxReach) * speed;
		else if (besideContainer && reachIntoBottom > 0) velocity = Math.min(reachIntoBottom, maxReach) * speed;

		// Away from the edges, the loop rests until a move brings the pointer back
		if (velocity === 0) {
			stopLoop();
			return;
		}
		animationFrame = requestAnimationFrame(frame);

		// Scaled by the frame's duration so the speed doesn't depend on the frame rate, where the first frame only starts the clock
		const seconds = previousTimestamp === undefined ? 0 : (timestamp - previousTimestamp) / 1000;
		previousTimestamp = timestamp;
		unscrolled += velocity * SPEED_PER_PIXEL_OF_REACH * seconds;

		const scrollTopBefore = container.scrollTop;
		container.scrollTop += Math.trunc(unscrolled);
		unscrolled -= Math.trunc(unscrolled);
		if (container.scrollTop !== scrollTopBefore) onScroll();
	};

	return {
		update: (dragContainer: HTMLElement, dragClientX: number, dragClientY: number, dragSpeed: number) => {
			container = dragContainer;
			clientX = dragClientX;
			clientY = dragClientY;
			speed = dragSpeed;
			if (animationFrame === undefined) animationFrame = requestAnimationFrame(frame);
		},
		stop: () => {
			stopLoop();
			container = undefined;
		},
	};
}

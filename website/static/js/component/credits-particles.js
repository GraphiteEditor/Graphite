/**
 * @typedef {{ left: number, top: number, width: number, height: number, em: number }} ParticleArea
 * The name's box within its canvases, which reach past it on every side, along with its font size.
 *
 * @typedef {{
 *   x: number, y: number, vx: number, vy: number, phase: number, phaseSpeed: number,
 *   age: number, life: number, size: number, angle: number, spin: number, seed: number, slot: number, driftX: number, color: string, front: boolean, shape: ParticleShape,
 * }} Particle
 * Positions and sizes are in CSS pixels, speeds are per second, and angles are in radians. A `slot` places a particle among a motion's steady count.
 * Its `driftX` is how fast it has lately been moving sideways, however its motion moves it.
 *
 * @typedef {{
 *   colors: string[],
 *   size: [number, number],
 *   tilt: number,
 *   spin: number,
 *   density: number,
 *   draw: (context: CanvasRenderingContext2D, particle: Particle, radius: number, area: ParticleArea) => void,
 * }} ParticleShape
 * What each particle looks like, drawn around the origin and already turned to the particle's angle, in the member's particle colors or else its own.
 * Its size is in ems, its tilt and spin are the most either way, and its density scales how many of it a motion makes.
 *
 * @typedef {{
 *   rate?: number,
 *   count?: number,
 *   group?: number,
 *   spawn: (area: ParticleArea, index: number, origin: { x: number, y: number }, count: number) => Partial<Particle>,
 *   update?: (particle: Particle, seconds: number, area: ParticleArea) => void,
 *   envelope: (particle: Particle) => [scale: number, opacity: number],
 *   trail?: (particle: Particle, area: ParticleArea) => { x: number, y: number }[],
 * }} ParticleMotion
 * Where the particles appear, how they move, and how long they last.
 * It makes a `rate` of them per second for a name eight letters wide, or keeps a steady `count` alive, with at least one for each chosen shape.
 * A `group` makes that many at once, sharing an `origin` picked at random within the name's box, in fractions of its width and height.
 */

// A pause like a hidden tab resumes where it left off, rather than catching up all at once
const PARTICLES_LONGEST_STEP_SECONDS = 0.1;

// With reduced motion, each effect is shown as a still frame from this far in
const PARTICLES_STILL_FRAME_SECONDS = 2;

const PARTICLES_LIMIT = 150;

// A maple leaf, traced from its top tip down the right side to the end of its stem and back up the left
const MAPLE_LEAF = new Path2D(
	"M0,-231 C13,-196 24,-204 28,-210 C20,-172 49,-178 64,-191 C43,-172 48,-142 49,-136 C56,-140 80,-150 92,-161 C97,-150 115,-166 137,-178 " +
		"C110,-158 120,-140 125,-125 C112,-115 106,-98 115,-90 C122,-84 136,-86 141,-86 C119,-82 89,-58 91,-52 C100,-46 140,-21 158,-37 C145,-22 100,0 113,15 " +
		"C90,8 70,15 73,27 C35,18 7,4 2,2 C3,25 3,50 2,61 C1.5,62 0.5,62 0,62 " +
		"C-0.5,62 -1.5,62 -2,61 C-3,50 -3,25 -2,2 C-7,4 -35,18 -73,27 C-70,15 -90,8 -113,15 C-100,0 -145,-22 -158,-37 C-140,-21 -100,-46 -91,-52 " +
		"C-89,-58 -119,-82 -141,-86 C-136,-86 -122,-84 -115,-90 C-106,-98 -112,-115 -125,-125 C-120,-140 -110,-158 -137,-178 C-115,-166 -97,-150 -92,-161 " +
		"C-80,-150 -56,-140 -49,-136 C-48,-142 -43,-172 -64,-191 C-49,-178 -20,-172 -28,-210 C-24,-204 -13,-196 0,-231 Z",
);

// A cherry blossom petal reaching up from the flower's middle to its notched tip, about a unit long, and a leaf pointed at both ends
const BLOSSOM_PETAL = new Path2D("M0,-0.08 C0.2,-0.2 0.44,-0.45 0.4,-0.7 C0.37,-0.9 0.24,-1 0.14,-0.97 L0,-0.84 L-0.14,-0.97 C-0.24,-1 -0.37,-0.9 -0.4,-0.7 C-0.44,-0.45 -0.2,-0.2 0,-0.08 Z");
const BLOSSOM_LEAF = new Path2D("M0,-0.5 C0.28,-0.3 0.26,0.2 0,0.5 C-0.22,0.24 -0.25,-0.26 0,-0.5 Z");
const BLOSSOM_LEAF_COLORS = ["#c5dc4a", "#a8c83c", "#d6e566"];

// The right wings of a butterfly whose body runs down the middle, about two units across with both sides
const BUTTERFLY_FOREWING = new Path2D("M0.05,-0.18 C0.35,-0.75 0.85,-0.88 0.97,-0.62 C1.06,-0.4 0.86,-0.06 0.6,0.04 C0.4,0.1 0.15,0.05 0.05,0.02 Z");
const BUTTERFLY_HINDWING = new Path2D("M0.05,0 C0.45,-0.02 0.82,0.16 0.73,0.45 C0.66,0.7 0.36,0.8 0.18,0.62 C0.1,0.52 0.05,0.32 0.05,0.15 Z");

// Any shape pairs with any motion, which alone decides where particles go and how long they last
/** @type {Record<string, ParticleShape>} */
const PARTICLE_SHAPES = {
	hearts: {
		colors: ["#d2697c", "#e0435e", "#b83a5a"],
		size: [0.26, 0.42],
		tilt: 0.3,
		spin: 0,
		density: 0.55,
		draw: (context, particle, radius) => {
			// Two lobes meeting at a point, within a box about two units across
			context.scale(radius, radius);
			context.beginPath();
			context.moveTo(0, 1);
			context.bezierCurveTo(-1.2, 0.2, -0.9, -0.9, 0, -0.35);
			context.bezierCurveTo(0.9, -0.9, 1.2, 0.2, 0, 1);
			context.fillStyle = particle.color;
			context.fill();
		},
	},

	// Jagged zigzags that flicker, reshaped every frame
	bolts: {
		colors: ["#5468ff", "#7a5cff", "#3f8ff5"],
		size: [0.4, 0.85],
		tilt: Math.PI,
		spin: 0,
		density: 0.4,
		draw: (context, particle, radius, area) => {
			const segments = 6;
			context.beginPath();
			context.moveTo(-radius / 2, 0);
			for (let i = 1; i <= segments; i++) context.lineTo(-radius / 2 + (radius * i) / segments, i === segments ? 0 : random(-0.18, 0.18) * area.em);

			context.strokeStyle = particle.color;
			context.lineJoin = "round";
			context.lineCap = "round";
			const opacity = context.globalAlpha * random(0.4, 1);
			context.globalAlpha = opacity * 0.2;
			context.lineWidth = 0.2 * area.em;
			context.stroke();
			context.globalAlpha = opacity;
			context.lineWidth = Math.max(1, 0.055 * area.em);
			context.stroke();
		},
	},

	// Dots that glow softly around their edges
	orbs: {
		colors: ["#3d7bd9", "#5b9bd5", "#e8a33d"],
		size: [0.08, 0.15],
		tilt: 0,
		spin: 0,
		density: 1.3,
		draw: (context, particle, radius) => {
			context.fillStyle = particle.color;
			const opacity = context.globalAlpha;
			context.globalAlpha = opacity * 0.25;
			circle(context, 0, 0, radius * 2.4);
			context.globalAlpha = opacity;
			circle(context, 0, 0, radius);
		},
	},

	// Bubbles with a glint, which pop at the end of their life
	bubbles: {
		colors: ["#4f8fa8", "#6fb3c9", "#4fa58f"],
		size: [0.14, 0.3],
		tilt: 0,
		spin: 0,
		density: 0.7,
		draw: (context, particle, radius, area) => {
			context.strokeStyle = particle.color;
			context.fillStyle = particle.color;
			context.lineWidth = Math.max(1, 0.045 * area.em);
			const opacity = context.globalAlpha;

			// The last moment is a ring bursting outward
			const popping = (lifeFraction(particle) - 0.88) / 0.12;
			if (popping > 0) {
				context.globalAlpha = opacity * (1 - popping);
				context.beginPath();
				context.arc(0, 0, radius * (1 + popping * 0.8), 0, Math.PI * 2);
				context.stroke();
				return;
			}

			context.globalAlpha = opacity * 0.12;
			circle(context, 0, 0, radius);
			context.globalAlpha = opacity * 0.85;
			context.beginPath();
			context.arc(0, 0, radius, 0, Math.PI * 2);
			context.stroke();

			// A glint on the upper left
			context.globalAlpha = opacity * 0.6;
			context.beginPath();
			context.arc(0, 0, radius * 0.62, Math.PI * 1.1, Math.PI * 1.4);
			context.stroke();
		},
	},

	// Paper scraps that flip over and over
	confetti: {
		colors: ["#d2697c", "#e8a33d", "#4fa58f", "#5b7bd5", "#cd8f7a"],
		size: [0.2, 0.3],
		tilt: Math.PI,
		spin: 5,
		density: 1.2,
		draw: (context, particle, radius) => {
			// Squashing one axis back and forth reads as the scrap flipping
			context.scale(1, Math.cos(particle.age * 7 + particle.seed));
			context.fillStyle = particle.color;
			context.fillRect(-radius / 2, -radius * 0.3, radius, radius * 0.6);
		},
	},

	// Four-pointed stars
	stars: {
		colors: ["#e8b33c", "#f2cf63", "#cd8f7a"],
		size: [0.22, 0.42],
		tilt: Math.PI / 4,
		spin: 1.5,
		density: 1,
		draw: (context, particle, radius) => {
			context.beginPath();
			context.moveTo(0, -radius);
			context.quadraticCurveTo(0, 0, radius, 0);
			context.quadraticCurveTo(0, 0, 0, radius);
			context.quadraticCurveTo(0, 0, -radius, 0);
			context.quadraticCurveTo(0, 0, 0, -radius);
			context.fillStyle = particle.color;
			context.fill();
		},
	},

	// Bézier segments with their anchors and handles, as drawn by Graphite's Pen tool, flexing as they go
	vectors: {
		colors: ["#00a8ff"],
		size: [0.7, 1],
		tilt: Math.PI,
		spin: 0.4,
		density: 0.5,
		draw: (context, particle, radius, area) => {
			const half = radius / 2;
			const handleLength = radius * 0.45;
			const startAngle = Math.sin(particle.age * 1.8 + particle.seed) * 0.9 - 0.5;
			const endAngle = Math.PI + Math.sin(particle.age * 1.5 + particle.seed * 2) * 0.9 + 0.5;
			const anchors = [
				{ x: -half, y: 0 },
				{ x: half, y: 0 },
			];
			const handles = [
				{ x: -half + Math.cos(startAngle) * handleLength, y: Math.sin(startAngle) * handleLength },
				{ x: half + Math.cos(endAngle) * handleLength, y: Math.sin(endAngle) * handleLength },
			];
			context.strokeStyle = particle.color;
			context.fillStyle = particle.color;

			// Handle lines, then the curve drawn on from its start, then the handle and anchor points over both
			context.lineWidth = Math.max(1, 0.03 * area.em);
			context.beginPath();
			anchors.forEach((anchor, i) => {
				context.moveTo(anchor.x, anchor.y);
				context.lineTo(handles[i].x, handles[i].y);
			});
			context.stroke();

			const drawn = Math.min(1, particle.age * 1.2);
			context.lineWidth = Math.max(1.2, 0.06 * area.em);
			context.beginPath();
			for (let i = 0; i <= 20; i++) {
				const t = (i / 20) * drawn;
				const x = cubicBezier(anchors[0].x, handles[0].x, handles[1].x, anchors[1].x, t);
				const y = cubicBezier(anchors[0].y, handles[0].y, handles[1].y, anchors[1].y, t);
				if (i === 0) context.moveTo(x, y);
				else context.lineTo(x, y);
			}
			context.stroke();

			handles.forEach((handle) => circle(context, handle.x, handle.y, 0.06 * area.em));
			const anchorSide = 0.15 * area.em;
			context.fillStyle = "white";
			anchors.forEach((anchor) => {
				context.fillRect(anchor.x - anchorSide / 2, anchor.y - anchorSide / 2, anchorSide, anchorSide);
				context.strokeRect(anchor.x - anchorSide / 2, anchor.y - anchorSide / 2, anchorSide, anchorSide);
			});
		},
	},

	// Bats seen from the front, beating without a pause, folding their wings back up and spreading them on the way down
	bats: {
		colors: ["#3d3547", "#4a3b5c", "#2f2a3a"],
		size: [0.24, 0.34],
		tilt: 0.25,
		spin: 0,
		density: 0.6,
		draw: (context, particle, radius, area) => {
			const time = particle.age + particle.seed;
			const opacity = context.globalAlpha;

			// The beat quickens and slows a little, like a bat's fluttering hunt
			const beat = time * 26 + 0.8 * Math.sin(time * 2.3);
			const elevation = 0.2 + 0.7 * Math.cos(beat);
			// Fullest partway down and most folded partway back up
			const extension = 0.775 + 0.225 * Math.sin(beat);

			// Each downstroke lifts the body a little, and the whole bat banks toward where it's heading
			context.translate(0, -Math.sin(beat) * 0.06 * radius);
			context.rotate(Math.max(-0.5, Math.min(0.5, (particle.driftX / area.em) * 0.25)));
			context.scale(radius, radius);
			context.fillStyle = particle.color;

			// The arm swings about the shoulder while the membrane stays fixed at the ankle, and the fingers fold back from the wrist as the wing closes
			const shoulder = { x: 0.06, y: -0.1 };
			const ankle = { x: 0.08, y: 0.34 };
			const raise = (/** @type {{ x: number, y: number }} */ point) => {
				const x = point.x - shoulder.x;
				const y = point.y - shoulder.y;
				return { x: shoulder.x + x * Math.cos(elevation) + y * Math.sin(elevation), y: shoulder.y - x * Math.sin(elevation) + y * Math.cos(elevation) };
			};
			const wrist = { x: 0.45 * (0.8 + 0.2 * extension), y: -0.14 };
			const finger = (/** @type {number} */ x, /** @type {number} */ y) => {
				const angle = Math.atan2(y, x) + (1 - extension) * 1.3;
				const length = Math.hypot(x, y) * (0.7 + 0.3 * extension);
				return raise({ x: wrist.x + Math.cos(angle) * length, y: wrist.y + Math.sin(angle) * length });
			};
			const tips = [finger(0.55, 0.12), finger(0.39, 0.44), finger(0.07, 0.58)];
			const raisedWrist = raise(wrist);

			// The leading edge arches over the forearm, and the trailing edge scallops inward between each pair of fingers
			const wing = new Path2D();
			const scallop = (/** @type {{ x: number, y: number }} */ from, /** @type {{ x: number, y: number }} */ to, /** @type {number} */ depth) => {
				const middle = { x: (from.x + to.x) / 2, y: (from.y + to.y) / 2 };
				wing.quadraticCurveTo(middle.x + (raisedWrist.x - middle.x) * depth, middle.y + (raisedWrist.y - middle.y) * depth, to.x, to.y);
			};
			const arch = raise({ x: (shoulder.x + wrist.x) / 2, y: wrist.y - 0.07 });
			wing.moveTo(shoulder.x, shoulder.y);
			wing.quadraticCurveTo(arch.x, arch.y, raisedWrist.x, raisedWrist.y);
			wing.lineTo(tips[0].x, tips[0].y);
			scallop(tips[0], tips[1], 0.35);
			scallop(tips[1], tips[2], 0.35);
			scallop(tips[2], ankle, 0.3);
			wing.closePath();

			[-1, 1].forEach((side) => {
				context.save();
				context.scale(side, 1);
				context.globalAlpha = opacity * 0.9;
				context.fill(wing);

				// Shaded toward the body, with the arm and finger bones showing through the membrane where they're big enough to see
				context.clip(wing);
				const root = context.createRadialGradient(0, 0, 0, 0, 0, 0.6);
				root.addColorStop(0, "rgb(0 0 0 / 0.3)");
				root.addColorStop(1, "rgb(0 0 0 / 0)");
				context.fillStyle = root;
				context.fill(wing);
				if (radius > 7) {
					context.globalAlpha = opacity * 0.45;
					context.strokeStyle = "black";
					context.lineWidth = 0.035;
					context.lineCap = "round";
					context.beginPath();
					context.moveTo(shoulder.x, shoulder.y);
					context.lineTo(raisedWrist.x, raisedWrist.y);
					tips.forEach((tip) => {
						context.moveTo(raisedWrist.x, raisedWrist.y);
						context.lineTo(tip.x, tip.y);
					});
					context.stroke();
				}
				context.restore();
			});

			// The body and head, with pointed ears
			context.globalAlpha = opacity;
			context.beginPath();
			context.ellipse(0, 0.1, 0.11, 0.24, 0, 0, Math.PI * 2);
			context.fill();
			context.beginPath();
			context.arc(0, -0.17, 0.1, 0, Math.PI * 2);
			context.fill();
			context.beginPath();
			[-1, 1].forEach((side) => {
				context.moveTo(side * 0.02, -0.23);
				context.lineTo(side * 0.11, -0.4);
				context.lineTo(side * 0.1, -0.19);
			});
			context.fill();
		},
	},

	// Whole cherry blossoms facing the viewer, deepening toward the middle, where white stamens radiate if they're big enough to see
	blossoms: {
		colors: ["#f2849e", "#f5a3bf", "#f9c0d3", "#ee93ad"],
		size: [0.2, 0.3],
		tilt: Math.PI,
		spin: 0.6,
		density: 0.8,
		draw: (context, particle, radius) => {
			context.scale(radius, radius);
			context.fillStyle = particle.color;
			for (let i = 0; i < 5; i++) {
				context.fill(BLOSSOM_PETAL);
				context.rotate((Math.PI * 2) / 5);
			}
			context.fillStyle = "rgb(170 30 70 / 0.22)";
			circle(context, 0, 0, 0.22);
			if (radius <= 6) return;

			context.strokeStyle = "white";
			context.fillStyle = "white";
			context.lineWidth = 0.045;
			for (let i = 0; i < 7; i++) {
				const angle = (i / 7) * Math.PI * 2;
				context.beginPath();
				context.moveTo(0, 0);
				context.lineTo(Math.cos(angle) * 0.27, Math.sin(angle) * 0.27);
				context.stroke();
				circle(context, Math.cos(angle) * 0.3, Math.sin(angle) * 0.3, 0.045);
			}
		},
	},

	// Butterflies seen from above, their beat quickening into bursts and slowing into lazy rests, banking into their turns
	butterflies: {
		colors: ["#e8803d", "#5b7bd5", "#7a5cff", "#4fa58f"],
		size: [0.24, 0.34],
		tilt: 0.35,
		spin: 0,
		density: 0.6,
		draw: (context, particle, radius, area) => {
			const time = particle.age + particle.seed;
			const opacity = context.globalAlpha;

			// The beat's rate swells and eases on a curve that lingers low, so the wings spend longer in slow rests than in quick bursts
			const surge = ((1 + Math.sin(time * 1.7)) / 2) ** 2;
			const resting = 1 - surge;

			// How far through its beats the butterfly is, which sums its changing rate over time so the wings never jump
			const slowest = Math.PI * 2.4;
			const quickest = Math.PI * 9;
			const beat = (/** @type {number} */ at) => {
				// The integral of the surge curve, which expands to 3/8 + sin(x) / 2 - cos(2x) / 8
				const surged = (3 * at) / 8 - Math.cos(1.7 * at) / (2 * 1.7) - Math.sin(3.4 * at) / (16 * 1.7);
				return slowest * at + (quickest - slowest) * surged;
			};
			const stroke = (/** @type {number} */ lag) => Math.cos(beat(time - lag));

			// Resting beats stay shallow and near open, while the quick ones close the wings almost shut
			const openness = (/** @type {number} */ lag) => 0.18 + 0.82 * ((0.5 + 0.5 * stroke(lag)) * (1 - 0.5 * resting) + 0.5 * resting);

			// Each downstroke lifts the body, less so in the gentle beats, and the whole butterfly banks toward where it's heading
			context.translate(0, (1 - 0.7 * resting) * stroke(0) * 0.06 * radius);
			context.rotate(Math.max(-0.5, Math.min(0.5, (particle.driftX / area.em) * 0.25)));
			context.scale(radius, radius);
			context.lineJoin = "round";

			// One wing seen turned partway up around the body, so its width shrinks toward the closed stroke
			const wing = (/** @type {Path2D} */ shape, /** @type {number} */ side, /** @type {number} */ open, /** @type {number} */ tint) => {
				context.save();
				context.scale(side * open, 1);
				context.fillStyle = particle.color;
				context.fill(shape);

				// Shaded where the wing meets the body, banded along its outer edge, and darker at the tip of the forewing
				context.clip(shape);
				const root = context.createRadialGradient(0, 0, 0, 0, 0, 0.55);
				root.addColorStop(0, "rgb(0 0 0 / 0.3)");
				root.addColorStop(1, "rgb(0 0 0 / 0)");
				context.fillStyle = root;
				context.fill(shape);
				context.globalAlpha = opacity * 0.4;
				context.strokeStyle = "black";
				context.lineWidth = 0.11;
				context.stroke(shape);
				context.globalAlpha = opacity * tint;
				context.fillStyle = "black";
				context.beginPath();
				context.ellipse(0.95, -0.62, 0.38, 0.3, -0.5, 0, Math.PI * 2);
				context.fill();

				// Pale spots near the tip, only where they're big enough to see
				if (tint > 0 && radius * open > 7) {
					context.globalAlpha = opacity * 0.85;
					context.fillStyle = "white";
					[
						[0.78, -0.62, 0.05],
						[0.88, -0.5, 0.04],
						[0.7, -0.7, 0.035],
					].forEach(([x = 0, y = 0, size = 0]) => {
						context.beginPath();
						context.arc(x, y, size, 0, Math.PI * 2);
						context.fill();
					});
				}
				context.restore();
			};

			// Hindwings trail the forewings slightly through each stroke, and sit beneath them
			[-1, 1].forEach((side) => wing(BUTTERFLY_HINDWING, side, openness(0.012), 0));
			[-1, 1].forEach((side) => wing(BUTTERFLY_FOREWING, side, openness(0), 0.35));

			// The body, with clubbed antennae reaching forward from the head
			context.globalAlpha = opacity;
			context.fillStyle = "#2b2422";
			context.strokeStyle = "#2b2422";
			context.beginPath();
			context.ellipse(0, -0.12, 0.07, 0.16, 0, 0, Math.PI * 2);
			context.ellipse(0, 0.27, 0.05, 0.3, 0, 0, Math.PI * 2);
			context.fill();
			context.beginPath();
			context.arc(0, -0.33, 0.065, 0, Math.PI * 2);
			context.fill();
			context.lineWidth = Math.max(0.035, 0.6 / radius);
			[-1, 1].forEach((side) => {
				context.beginPath();
				context.moveTo(side * 0.03, -0.37);
				context.quadraticCurveTo(side * 0.06, -0.6, side * 0.2, -0.7);
				context.stroke();
				context.beginPath();
				context.arc(side * 0.2, -0.7, 0.035, 0, Math.PI * 2);
				context.fill();
			});
		},
	},

	// Fireworks bursting over and over, each burst a different kind of shell, slowing as they spread and sinking as they fade
	fireworks: {
		colors: ["#e8a33d", "#d2697c", "#5b7bd5", "#4fa58f"],
		size: [0.5, 0.7],
		tilt: 0,
		spin: 0,
		density: 0.35,
		draw: (context, particle, radius) => {
			// Timed from when the particle appears, so its first burst is seen from the start
			const duration = 1.2 + (particle.seed % 1) * 0.4;
			const burst = Math.floor(particle.age / duration);
			const progress = (particle.age / duration) % 1;
			const kind = (Math.floor(particle.seed) + burst) % 4;
			const turn = particle.seed + burst * 2.4;

			// Air drag slows the sparks quickly while they soon settle into falling, staying bright until they burn out toward the end
			const spread = (/** @type {number} */ at) => 1 - (1 - Math.max(0, at)) ** 3;
			const sink = (/** @type {number} */ at) => Math.max(0, at) ** 1.6 * (kind === 1 ? 1.2 : 0.9);
			const fade = 1 - progress ** 2.2;
			const opacity = context.globalAlpha;

			context.scale(radius, radius);
			context.fillStyle = particle.color;
			context.strokeStyle = particle.color;
			context.lineCap = "round";

			// The break itself, a white flash at the center
			if (progress < 0.12) {
				context.globalAlpha = opacity * (1 - progress / 0.12);
				context.fillStyle = "white";
				circle(context, 0, 0, 0.3 * (0.5 + progress / 0.12));
				context.fillStyle = particle.color;
			}

			// Each spark flies out along its own direction, around a shell that may be smaller, flattened, or tilted, and falls straight down whatever its shell
			const position = (/** @type {number} */ angle, /** @type {number} */ at, /** @type {{ size?: number, squash?: number, tilt?: number }} */ shell = {}) => {
				const { size = 1, squash = 1, tilt = 0 } = shell;
				const x = Math.cos(angle) * spread(at) * size;
				const y = Math.sin(angle) * spread(at) * size * squash;
				return { x: x * Math.cos(tilt) - y * Math.sin(tilt), y: x * Math.sin(tilt) + y * Math.cos(tilt) + sink(at) };
			};
			const trail = (/** @type {(at: number) => { x: number, y: number }} */ path, /** @type {number} */ length) => {
				context.beginPath();
				for (let i = 0; i <= 4; i++) {
					const point = path(progress - length * (1 - i / 4));
					if (i === 0) context.moveTo(point.x, point.y);
					else context.lineTo(point.x, point.y);
				}
				context.stroke();
			};
			context.globalAlpha = opacity * fade;

			// A peony, a sphere of sparks seen face-on, so they fill it and crowd toward its rim, glittering as they burn out
			if (kind === 0) {
				const sparks = 26;
				for (let i = 0; i < sparks; i++) {
					// Spread evenly over the sphere by stepping the golden angle around while sweeping from pole to pole
					const depth = 1 - (2 * (i + 0.5)) / sparks;
					const point = position(turn + i * 2.39996, progress, { size: Math.sqrt(1 - depth * depth) });
					const twinkle = progress > 0.55 && Math.sin(particle.age * 45 + i * 2.1) < 0 ? 0.25 : 1;
					context.globalAlpha = opacity * fade * twinkle;
					circle(context, point.x, point.y, 0.07 * (0.4 + 0.6 * fade));
				}
			}

			// A willow, whose long trails droop as they hang in the air
			if (kind === 1) {
				context.lineWidth = 0.05;
				for (let i = 0; i < 12; i++) {
					const angle = turn + (i / 12) * Math.PI * 2;
					trail((at) => position(angle, at), 0.3);
				}
			}

			// A ring seen tilted, with a smaller ring of white inside it
			if (kind === 2) {
				const tilt = Math.sin(turn) * 0.6;
				for (let i = 0; i < 18; i++) {
					const point = position((i / 18) * Math.PI * 2, progress, { squash: 0.45, tilt });
					circle(context, point.x, point.y, 0.07 * (0.4 + 0.6 * fade));
				}
				context.fillStyle = "white";
				for (let i = 0; i < 8; i++) {
					const point = position((i / 8) * Math.PI * 2, progress, { size: 0.45, squash: 0.45, tilt });
					circle(context, point.x, point.y, 0.05 * fade);
				}
			}

			// A crossette, whose few stars each split in two halfway out
			if (kind === 3) {
				context.lineWidth = 0.06;
				const star = (/** @type {{ x: number, y: number }} */ point) => circle(context, point.x, point.y, 0.06 * (0.5 + 0.5 * fade));
				for (let i = 0; i < 5; i++) {
					const angle = turn + (i / 5) * Math.PI * 2;
					if (progress < 0.45) {
						trail((at) => position(angle, at), 0.15);
						star(position(angle, progress));
						continue;
					}

					// Past the split, each half carries on from where its star broke apart
					const split = position(angle, 0.45);
					[-0.55, 0.55].forEach((offset) => {
						const branch = (/** @type {number} */ at) => {
							if (at <= 0.45) return position(angle, at);
							const beyond = (spread(at) - spread(0.45)) * 1.6;
							return { x: split.x + Math.cos(angle + offset) * beyond, y: split.y + Math.sin(angle + offset) * beyond + sink(at) - sink(0.45) };
						};
						trail(branch, 0.15);
						star(branch(progress));
					});
				}
			}
		},
	},

	// Flames rippled by turbulence that rises through them like hot air, leaning away from their movement and now and then tearing a lick off the tip
	flames: {
		colors: ["#e8592b", "#f08a24", "#d2452b"],
		size: [0.22, 0.34],
		tilt: 0,
		spin: 0,
		density: 0.9,
		draw: (context, particle, radius, area) => {
			const time = particle.age + particle.seed;
			const opacity = context.globalAlpha;
			const base = radius * 0.9;
			const width = radius * 0.62;
			const height = radius * 2.5;

			// Drag bends the upper flame away from the way it's moving
			const lean = Math.max(-1, Math.min(1, (-particle.driftX / area.em) * 0.35)) * radius;

			// The middle of the flame some fraction of the way up, swayed by ripples that travel upward and grow toward the free tip
			const middle = (/** @type {number} */ fraction, /** @type {number} */ calm, /** @type {number} */ phase) => {
				const ripple = Math.sin(fraction * 7 - time * 9 + phase) * 0.55 + Math.sin(fraction * 13 - time * 15.5 + phase * 2) * 0.25;
				return ripple * fraction ** 1.5 * width * (1 - calm) + lean * fraction ** 2;
			};

			// One layer of the flame, traced up its right side and back down its left, then smoothed through the midpoints between
			const layer = (/** @type {number} */ widthScale, /** @type {number} */ heightScale, /** @type {number} */ calm, /** @type {number} */ phase) => {
				const steps = 16;
				/** @type {[number, number][]} */
				const points = [];
				const trace = (/** @type {number} */ fraction, /** @type {number} */ side) => {
					// Widest a third of the way up, rounded at the bottom and tapering to the tip, with bulges rising along the sides
					const profile = 2.8 * Math.sqrt(fraction) * (1 - fraction) ** 1.2;
					const bulge = 1 + 0.15 * Math.sin(fraction * 9 - time * 12 + phase) * (1 - calm);
					points.push([middle(fraction, calm, phase) + side * width * widthScale * profile * bulge, base - fraction * height * heightScale]);
				};
				for (let i = 0; i <= steps; i++) trace(i / steps, 1);
				for (let i = steps - 1; i > 0; i--) trace(i / steps, -1);

				const midpoint = (/** @type {[number, number]} */ a, /** @type {[number, number]} */ b) => /** @type {[number, number]} */ ([(a[0] + b[0]) / 2, (a[1] + b[1]) / 2]);
				const last = points[points.length - 1];
				const first = points[0];
				if (!last || !first) return;
				context.beginPath();
				context.moveTo(...midpoint(last, first));
				points.forEach((point, i) => context.quadraticCurveTo(point[0], point[1], ...midpoint(point, points[(i + 1) % points.length] || first)));
				context.fill();
			};

			// Each layer fades out toward the tip, where the flame burns away into the air
			const fill = (/** @type {string} */ bottom, /** @type {string} */ top, /** @type {number} */ heightScale) => {
				const gradient = context.createLinearGradient(0, base, 0, base - height * heightScale);
				gradient.addColorStop(0, bottom);
				gradient.addColorStop(0.55, bottom);
				gradient.addColorStop(1, top);
				context.fillStyle = gradient;
			};

			// A soft glow of the flame's own color
			const glowY = base - height * 0.35;
			const glow = context.createRadialGradient(0, glowY, 0, 0, glowY, height * 0.75);
			glow.addColorStop(0, `${particle.color}40`);
			glow.addColorStop(1, `${particle.color}00`);
			context.fillStyle = glow;
			circle(context, 0, glowY, height * 0.75);

			// The outer fire, then hotter and steadier layers toward the core
			fill(particle.color, `${particle.color}33`, 1);
			layer(1, 1, 0, 0);
			fill("rgb(255 255 255 / 0.4)", "rgb(255 255 255 / 0)", 0.72);
			layer(0.6, 0.72, 0.35, 1.3);
			fill("rgb(255 255 255 / 0.8)", "rgb(255 255 255 / 0.1)", 0.42);
			layer(0.34, 0.42, 0.6, 2.6);

			// Now and then the tip pinches off a lick that rises and burns away
			const tearing = (time * 0.8 + particle.seed * 0.37) % 1;
			if (tearing < 0.45) {
				const progress = tearing / 0.45;
				const size = width * 0.32 * (1 - progress);
				const x = middle(1, 0, 0) + Math.sin(time * 6) * width * 0.2;
				const y = base - height * (1.02 + progress * 0.4);
				const curl = Math.sin(time * 5) * size * 0.6;

				// A wisp tapered at both ends, its lower end trailing toward the flame it tore from
				context.globalAlpha = opacity * (1 - progress) * 0.8;
				context.fillStyle = particle.color;
				context.beginPath();
				context.moveTo(x + curl, y - size * 1.6);
				context.quadraticCurveTo(x + size * 0.9, y - size * 0.1, x - curl * 0.5, y + size * 1.4);
				context.quadraticCurveTo(x - size * 0.9, y - size * 0.1, x + curl, y - size * 1.6);
				context.fill();
			}
		},
	},

	// Cut gems, their facets catching the light
	gems: {
		colors: ["#4fa58f", "#5b7bd5", "#d2697c"],
		size: [0.16, 0.26],
		tilt: 0.3,
		spin: 0.8,
		density: 0.8,
		draw: (context, particle, radius, area) => {
			context.beginPath();
			context.moveTo(-0.5 * radius, -0.55 * radius);
			context.lineTo(0.5 * radius, -0.55 * radius);
			context.lineTo(radius, -0.15 * radius);
			context.lineTo(0, radius);
			context.lineTo(-radius, -0.15 * radius);
			context.closePath();
			context.fillStyle = particle.color;
			context.fill();

			// The girdle across the widest part, and the facets running down from the top
			context.beginPath();
			context.moveTo(-radius, -0.15 * radius);
			context.lineTo(radius, -0.15 * radius);
			context.moveTo(-0.5 * radius, -0.55 * radius);
			context.lineTo(0, radius);
			context.lineTo(0.5 * radius, -0.55 * radius);
			context.strokeStyle = "rgb(255 255 255 / 0.55)";
			context.lineWidth = Math.max(1, 0.025 * area.em);
			context.stroke();
		},
	},

	// The lit part of the moon in its phases from a thin crescent up to half
	crescents: {
		colors: ["#e8b33c", "#cd8f7a", "#495875"],
		size: [0.16, 0.26],
		tilt: 0.5,
		spin: 0.6,
		density: 0.7,
		draw: (context, particle, radius) => {
			// How much of the face is lit, picked per particle from its seed
			const fullness = 0.08 + 0.42 * (Math.abs(Math.sin(particle.seed * 12.9898) * 43758.5453) % 1);

			// Down the lit half of the rim, then back up the line between light and dark, which bows into the lit side less the fuller it is
			context.beginPath();
			context.arc(0, 0, radius, -Math.PI / 2, Math.PI / 2);
			context.ellipse(0, 0, (1 - 2 * fullness) * radius, radius, 0, Math.PI / 2, -Math.PI / 2, true);
			context.closePath();
			context.fillStyle = particle.color;
			context.fill();
		},
	},

	// Eighth notes, some alone and some beamed in pairs
	notes: {
		colors: ["#16323f", "#5b7bd5", "#803847"],
		size: [0.22, 0.32],
		tilt: 0.3,
		spin: 0,
		density: 0.6,
		draw: (context, particle, radius) => {
			const head = (/** @type {number} */ x, /** @type {number} */ y, /** @type {number} */ scale) => {
				context.beginPath();
				context.ellipse(x * radius, y * radius, 0.32 * scale * radius, 0.24 * scale * radius, -0.35, 0, Math.PI * 2);
				context.fill();
			};
			context.fillStyle = particle.color;
			context.strokeStyle = particle.color;
			context.lineWidth = radius * 0.12;

			// Two smaller heads, each with a stem up to the beam slanting across their tops
			if (Math.floor(particle.seed) % 2 === 1) {
				head(-0.5, 0.6, 0.8);
				head(0.45, 0.45, 0.8);

				context.beginPath();
				context.moveTo(-radius * 0.26, radius * 0.55);
				context.lineTo(-radius * 0.26, -radius * 0.8);
				context.moveTo(radius * 0.69, radius * 0.4);
				context.lineTo(radius * 0.69, -radius * 0.95);
				context.stroke();

				context.lineWidth = radius * 0.2;
				context.beginPath();
				context.moveTo(-radius * 0.32, -radius * 0.72);
				context.lineTo(radius * 0.75, -radius * 0.87);
				context.stroke();
				return;
			}

			head(-0.25, 0.55, 1);

			// The stem rising from the head, with its flag curling down from the top
			context.lineCap = "round";
			context.beginPath();
			context.moveTo(radius * 0.05, radius * 0.5);
			context.lineTo(radius * 0.05, -radius * 0.85);
			context.bezierCurveTo(radius * 0.45, -radius * 0.6, radius * 0.55, -radius * 0.3, radius * 0.3, -radius * 0.05);
			context.stroke();
		},
	},

	// Leaves, turning over as they go
	leaves: {
		colors: ["#c2352b", "#e8803d", "#e8b33c"],
		size: [0.26, 0.38],
		tilt: 0.6,
		spin: 0.8,
		density: 0.6,
		draw: (context, particle, radius) => {
			turnOver(context, particle);

			// The outline is about 300 units across, centered 85 units below its origin
			context.scale(radius / 150, radius / 150);
			context.translate(0, 85);
			context.fillStyle = particle.color;
			context.fill(MAPLE_LEAF);
		},
	},

	// Loose cherry blossom petals and the odd fresh leaf, turning over around their length as they tumble
	petals: {
		colors: ["#f2849e", "#f5a3bf", "#f9c0d3", "#ee93ad"],
		size: [0.3, 0.45],
		tilt: Math.PI,
		spin: 1,
		density: 1,
		draw: (context, particle, radius) => {
			context.scale(radius, radius);
			turnOver(context, particle);

			// Three in five are petals, centered on their middle rather than where they met the flower, and the rest are leaves
			if (Math.floor(particle.seed * 13) % 5 < 3) {
				context.translate(0, 0.53);
				context.fillStyle = particle.color;
				context.fill(BLOSSOM_PETAL);
				return;
			}

			context.fillStyle = BLOSSOM_LEAF_COLORS[Math.floor(particle.seed) % BLOSSOM_LEAF_COLORS.length] || particle.color;
			context.fill(BLOSSOM_LEAF);
		},
	},

	// Crisp squares, kept upright like the pixels of an image
	pixels: {
		colors: ["#5b7bd5", "#4fa58f", "#e8a33d", "#d2697c"],
		size: [0.08, 0.14],
		tilt: 0,
		spin: 0,
		density: 1.4,
		draw: (context, particle, radius) => {
			context.fillStyle = particle.color;
			context.fillRect(-radius, -radius, radius * 2, radius * 2);
		},
	},

	// Six arms, each with a pair of short branches partway along it
	snowflakes: {
		colors: ["#6fb3c9", "#4f8fa8", "#8fb3e0"],
		size: [0.18, 0.3],
		tilt: Math.PI / 6,
		spin: 0.8,
		density: 0.7,
		draw: (context, particle, radius, area) => {
			turnOver(context, particle);

			context.beginPath();
			for (let i = 0; i < 6; i++) {
				context.moveTo(0, 0);
				context.lineTo(radius, 0);
				context.moveTo(radius * 0.55, 0);
				context.lineTo(radius * 0.8, -radius * 0.22);
				context.moveTo(radius * 0.55, 0);
				context.lineTo(radius * 0.8, radius * 0.22);
				context.rotate(Math.PI / 3);
			}
			context.strokeStyle = particle.color;
			context.lineWidth = Math.max(1, 0.035 * area.em);
			context.lineCap = "round";
			context.stroke();
		},
	},
};

// How many particles the motions with a steady count keep alive, unless more shapes are chosen
const PARTICLES_ENCIRCLING = 10;
const PARTICLES_LEVITATING = 7;
const PARTICLES_ORBITING = 6;
const ORBIT_PLANES = 3;
const PARTICLES_PER_BURST = 8;

/** @type {Record<string, ParticleMotion>} */
const PARTICLE_MOTIONS = {
	// Appearing in place around the name, swelling and shrinking away
	blink: {
		rate: 9,
		spawn: (area) => ({
			x: area.left + random(-0.4, 0.4) * area.em + Math.random() * area.width,
			y: area.top + random(-0.45, 0.45) * area.em + Math.random() * area.height,
			life: random(0.7, 1.3),
			front: Math.random() < 0.4,
		}),
		envelope: (particle) => [Math.sin(Math.PI * lifeFraction(particle)), 1],
	},

	// Dropping from above and bouncing on the bottom of the name, lower each time
	bounce: {
		rate: 6,
		spawn: (area) => ({
			x: area.left + Math.random() * area.width,
			y: area.top - random(0.3, 0.8) * area.em,
			vx: random(-0.35, 0.35) * area.em,
			life: random(1.8, 2.6),
			front: Math.random() < 0.5,
		}),
		update: (particle, seconds, area) => {
			particle.vy += 7 * area.em * seconds;

			const floor = area.top + area.height;
			if (particle.y > floor && particle.vy > 0) {
				particle.y = floor;
				particle.vy *= -0.55;
			}
		},
		envelope: (particle) => {
			const progress = lifeFraction(particle);
			return [1, Math.min(1, progress * 8, (1 - progress) * 4)];
		},
	},

	// Evenly spaced around the outside of the name, running along its edges and rounding its corners like marquee lights
	encircle: {
		count: PARTICLES_ENCIRCLING,
		spawn: (_area, index, _origin, count) => ({ life: Infinity, phase: index / count }),
		update: (particle, seconds, area) => {
			const path = encirclingPath(0.35 * area.em, 0.6 * area.em, area);

			// Each one's pace swells and slackens on its own rhythm, so neighbors bunch up and spread apart without overtaking
			const pace = 1 + 0.5 * Math.sin(particle.age * (0.5 + (particle.seed % 1) * 0.5) + particle.seed);
			particle.phase += ((1.2 * area.em) / path.length) * pace * seconds;

			const position = path.at(particle.phase);
			particle.x = position.x;
			particle.y = position.y;
		},
		envelope: () => [1, 1],
	},

	// Floating slowly up and around, fading in and out
	drift: {
		rate: 6,
		spawn: (area) => ({
			x: area.left + Math.random() * area.width,
			y: area.top + random(-0.2, 1.2) * area.height,
			vx: random(-0.12, 0.12) * area.em,
			vy: -random(0.1, 0.25) * area.em,
			life: random(2.2, 3.2),
			front: Math.random() < 0.2,
		}),
		envelope: (particle) => {
			const progress = lifeFraction(particle);
			return [1, Math.min(1, progress * 5, (1 - progress) * 4)];
		},
	},

	// Tumbling down across the name from above it
	fall: {
		rate: 3.5,
		spawn: (area) => {
			const fallSpeed = random(0.5, 0.9) * area.em;
			return {
				x: area.left + random(-0.4, 0.4) * area.em + Math.random() * area.width,
				y: area.top - random(0.2, 0.7) * area.em,
				vx: random(-0.25, 0.25) * area.em,
				vy: fallSpeed,
				life: (area.height + 1.4 * area.em) / fallSpeed,
				front: Math.random() < 0.5,
			};
		},
		update: (particle, seconds, area) => {
			particle.x += Math.sin(particle.age * 2.5 + particle.seed) * 0.3 * area.em * seconds;
		},
		envelope: (particle) => {
			const progress = lifeFraction(particle);
			return [1, Math.min(1, progress * 8, (1 - progress) * 5)];
		},
	},

	// Coming in from all around the name and shrinking away as they reach it
	gather: {
		rate: 9,
		spawn: (area) => {
			const angle = random(0, Math.PI * 2);
			const centerX = area.left + area.width / 2;
			const centerY = area.top + area.height / 2;
			const startX = centerX + Math.cos(angle) * (area.width / 2 + 0.85 * area.em);
			const startY = centerY + Math.sin(angle) * (area.height / 2 + 0.85 * area.em);
			const endX = centerX + (Math.cos(angle) * area.width) / 4;
			const endY = centerY + (Math.sin(angle) * area.height) / 4;
			const life = random(1.1, 1.6);
			return { x: startX, y: startY, vx: (endX - startX) / life, vy: (endY - startY) / life, life, front: Math.random() < 0.4 };
		},
		envelope: (particle) => {
			const progress = lifeFraction(particle);
			return [1 - 0.6 * progress ** 3, Math.min(1, progress * 4, (1 - progress) * 5)];
		},
	},

	// A row floating above the name, bobbing one after another like a wave
	levitate: {
		count: PARTICLES_LEVITATING,
		spawn: (_area, index, _origin, count) => ({ life: Infinity, slot: (index + 0.5) / count, phase: index * 0.9 }),
		update: (particle, _seconds, area) => {
			const bob = Math.sin(particle.phase + particle.age * 2.6);
			particle.x = area.left + particle.slot * area.width;
			particle.y = area.top - 0.3 * area.em + bob * 0.14 * area.em;
		},
		envelope: (particle) => [1 + 0.1 * Math.sin(particle.phase + particle.age * 2.6), 1],
	},

	// Circling the name on tilted orbits, passing in front of it and behind, each trailing where it just was
	orbit: {
		count: PARTICLES_ORBITING,
		// Taking the orbits in turn spaces out the particles that share one, which all keep its pace
		spawn: (_area, index, _origin, count) => ({
			life: Infinity,
			phase: (index * Math.PI * 2) / count,
			phaseSpeed: 1.1 + (index % ORBIT_PLANES) * 0.175,
			slot: index % ORBIT_PLANES,
		}),
		update: (particle, seconds, area) => {
			particle.phase += particle.phaseSpeed * seconds;
			const position = orbitPosition(particle.phase, particle.slot, area);
			particle.x = position.x;
			particle.y = position.y;
			particle.front = Math.sin(particle.phase) > 0;
		},
		// Nearer is bigger, which sells the depth of the orbit
		envelope: (particle) => [1 + 0.25 * Math.sin(particle.phase), 1],
		trail: (particle, area) => Array.from({ length: 6 }, (_, i) => orbitPosition(particle.phase - (i + 1) * 0.11, particle.slot, area)),
	},

	// Swaying upward from the name and fading out
	rise: {
		rate: 7.5,
		spawn: (area) => ({
			x: area.left + Math.random() * area.width,
			y: area.top + random(0.5, 1.1) * area.height,
			vx: random(-0.15, 0.15) * area.em,
			vy: -random(0.5, 0.9) * area.em,
			life: random(1, 1.8),
			front: Math.random() < 0.3,
		}),
		update: (particle, seconds, area) => {
			particle.x += Math.sin(particle.age * 3 + particle.seed) * 0.3 * area.em * seconds;
		},
		envelope: (particle) => {
			const progress = lifeFraction(particle);
			return [1 - (1 - Math.min(1, progress / 0.15)) ** 2, Math.min(1, (1 - progress) / 0.4)];
		},
	},

	// Bursting outward from points on the name, slowing to a stop as they fade
	burst: {
		rate: 14,
		group: PARTICLES_PER_BURST,
		spawn: (area, index, origin) => {
			const angle = (index / PARTICLES_PER_BURST) * Math.PI * 2 + random(-0.3, 0.3);
			const speed = random(1.1, 1.7) * area.em;
			return {
				x: area.left + origin.x * area.width,
				y: area.top + origin.y * area.height,
				vx: Math.cos(angle) * speed,
				vy: Math.sin(angle) * speed,
				life: random(0.9, 1.3),
				front: Math.random() < 0.4,
			};
		},
		update: (particle, seconds, area) => {
			const drag = Math.exp(-2.6 * seconds);
			particle.vx *= drag;
			particle.vy = particle.vy * drag + 0.8 * area.em * seconds;
		},
		envelope: (particle) => {
			const progress = lifeFraction(particle);
			return [Math.min(1, progress * 10), 1 - progress ** 2];
		},
	},

	// Climbing from below the name to above it while wrapping around it, passing in front and behind
	ascent: {
		rate: 7,
		spawn: (area) => ({
			life: random(1.8, 2.4),
			phase: random(0, Math.PI * 2),
			// Turns at a pace that keeps the same speed across any width of name
			phaseSpeed: ((2 * area.em) / (area.width / 2 + 0.4 * area.em)) * random(0.8, 1.2),
		}),
		update: (particle, seconds, area) => {
			particle.phase += particle.phaseSpeed * seconds;
			particle.x = area.left + area.width / 2 + Math.cos(particle.phase) * (area.width / 2 + 0.4 * area.em);
			particle.y = area.top + area.height + 0.4 * area.em - lifeFraction(particle) * (area.height + area.em);
			particle.front = Math.sin(particle.phase) > 0;
		},
		envelope: (particle) => {
			const progress = lifeFraction(particle);
			return [1 + 0.2 * Math.sin(particle.phase), Math.min(1, progress * 5, (1 - progress) * 5)];
		},
	},

	// Flowing along the name from start to end, weaving above and below its middle
	stream: {
		rate: 4,
		spawn: (area) => {
			const speed = random(1.4, 2) * area.em;
			return {
				x: area.left - 0.6 * area.em,
				vx: speed,
				life: (area.width + 1.2 * area.em) / speed,
				front: Math.random() < 0.5,
			};
		},
		update: (particle, _seconds, area) => {
			const along = (particle.x - area.left) / area.em;
			particle.y = area.top + area.height / 2 + Math.sin(along * 1.6 + particle.seed) * (area.height / 2 + 0.2 * area.em) * 0.9;
		},
		envelope: (particle) => {
			const progress = lifeFraction(particle);
			return [1, Math.min(1, progress * 6, (1 - progress) * 6)];
		},
	},

	// Whirling up from a tight point below the name into a funnel that widens above it, fading out at the top
	tornado: {
		rate: 10,
		spawn: () => ({
			life: random(1.6, 2.2),
			phase: random(0, Math.PI * 2),
			front: false,
		}),
		update: (particle, seconds, area) => {
			const progress = lifeFraction(particle);
			const narrowest = 0.15 * area.em;
			const radius = narrowest + (area.width / 2 + 0.6 * area.em - narrowest) * progress ** 1.5;

			// Spins fastest where the funnel is narrow, like a real tornado
			particle.phase += ((4 * area.em) / (radius + 0.5 * area.em)) * seconds;

			const depth = Math.sin(particle.phase);
			particle.x = area.left + area.width / 2 + Math.cos(particle.phase) * radius;
			particle.y = area.top + area.height + 0.5 * area.em - progress * (area.height + 1.3 * area.em) + depth * radius * 0.15;
			particle.front = depth > 0;
		},
		envelope: (particle) => {
			const progress = lifeFraction(particle);
			return [(0.5 + 0.5 * progress) * (1 + 0.2 * Math.sin(particle.phase)), Math.min(1, progress * 6, (1 - progress) * 3)];
		},
	},
};

// Every name marked with particles gets canvases behind and in front of it, animated only while on screen
window.addEventListener("DOMContentLoaded", () => {
	const hosts = Array.from(document.querySelectorAll("[data-name-particle-shapes]")).filter((host) => host instanceof HTMLElement);
	if (hosts.length === 0) return;

	const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
	const scenes = hosts.map(createParticleScene);

	let frameRequest = 0;
	let previousTime = 0;
	const animate = (/** @type {number} */ time) => {
		const seconds = Math.min((time - previousTime) / 1000, PARTICLES_LONGEST_STEP_SECONDS);
		previousTime = time;

		const active = scenes.filter((scene) => scene.active());
		active.forEach((scene) => {
			scene.step(seconds);
			scene.draw();
		});
		frameRequest = active.length > 0 ? window.requestAnimationFrame(animate) : 0;
	};

	// Reduced motion draws each scene once, and otherwise the animation runs while any scene is on screen
	const refresh = () => {
		if (reducedMotion.matches) {
			window.cancelAnimationFrame(frameRequest);
			frameRequest = 0;
			scenes.forEach((scene) => scene.still());
			return;
		}

		if (frameRequest || !scenes.some((scene) => scene.active())) return;
		previousTime = performance.now();
		frameRequest = window.requestAnimationFrame(animate);
	};
	scenes.forEach((scene) => scene.onChange(refresh));
	reducedMotion.addEventListener("change", refresh);
});

/** @param {HTMLElement} host */
function createParticleScene(host) {
	const [back, front] = ["back", "front"].map((layer) => {
		const canvas = document.createElement("canvas");
		canvas.className = `credits-particles ${layer}`;
		canvas.setAttribute("aria-hidden", "true");
		return canvas;
	});
	const backContext = back.getContext("2d");
	const frontContext = front.getContext("2d");

	/** @type {ParticleShape[]} */
	let shapes = [];
	/** @type {ParticleMotion | undefined} */
	let motion;
	let choices = "";
	/** @type {Particle[]} */
	let particles = [];
	/** @type {string[]} */
	let palette = [];
	/** @type {ParticleArea} */
	let area = { left: 0, top: 0, width: 0, height: 0, em: 16 };
	let spawnDebt = 0;
	let onScreen = false;
	let notify = () => {};

	const measure = () => {
		const ratio = window.devicePixelRatio || 1;
		[back, front].forEach((canvas) => {
			canvas.width = Math.round(canvas.clientWidth * ratio);
			canvas.height = Math.round(canvas.clientHeight * ratio);
			canvas.getContext("2d")?.setTransform(ratio, 0, 0, ratio, 0, 0);
		});

		const left = (back.clientWidth - host.clientWidth) / 2;
		const top = (back.clientHeight - host.clientHeight) / 2;
		area = { left, top, width: host.clientWidth, height: host.clientHeight, em: parseFloat(getComputedStyle(host).fontSize) };
	};

	// The choices and the member's colors are read from the host, which the membership page's preview changes as the member edits
	const configure = () => {
		const shapeNames = host.dataset.nameParticleShapes || "";
		const motionName = host.dataset.nameParticleMotion || "";
		if (`${shapeNames} ${motionName}` !== choices) {
			choices = `${shapeNames} ${motionName}`;
			shapes = shapeNames.split(",").flatMap((name) => PARTICLE_SHAPES[name] || []);
			motion = shapes.length > 0 ? PARTICLE_MOTIONS[motionName] : undefined;
			particles = [];
			spawnDebt = 0;
			if (motion) host.append(back, front);
			else [back, front].forEach((canvas) => canvas.remove());
		}
		if (!motion) return;

		const chosenPalette = getComputedStyle(host)
			.getPropertyValue("--particle-colors")
			.split(",")
			.map((color) => color.trim())
			.filter(Boolean);
		if (chosenPalette.join() !== palette.join()) {
			palette = chosenPalette;
			particles.forEach((particle, i) => {
				particle.color = colorFor(particle.shape, i);
			});
		}

		measure();
	};

	// The member's particle colors, or else the shape's own
	const colorFor = (/** @type {ParticleShape} */ shape, /** @type {number} */ index) => {
		const colors = palette.length > 0 ? palette : shape.colors;
		return colors[index % colors.length] || "";
	};

	// A steady count takes the shapes and colors in turn, so each one shows, while the rest are picked at random
	const spawn = (/** @type {ParticleMotion} */ chosenMotion, /** @type {number} */ index, origin = { x: 0.5, y: 0.5 }, count = 0) => {
		const turn = (/** @type {number} */ length) => (chosenMotion.count ? index % length : Math.floor(Math.random() * length));
		const shape = shapes[turn(shapes.length)] || PARTICLE_SHAPES.orbs;
		const colors = palette.length > 0 ? palette : shape.colors;
		return particle(shape, {
			size: random(...shape.size) * area.em,
			angle: random(-shape.tilt, shape.tilt),
			spin: random(-shape.spin, shape.spin),
			color: colors[turn(colors.length)] || "",
			...chosenMotion.spawn(area, index, origin, count),
		});
	};

	const step = (/** @type {number} */ seconds) => {
		if (!motion) return;

		if (motion.count) {
			const count = Math.max(motion.count, shapes.length);
			while (particles.length < count) particles.push(spawn(motion, particles.length, undefined, count));
		} else {
			const group = motion.group || 1;
			const density = shapes.reduce((sum, shape) => sum + shape.density, 0) / shapes.length;
			spawnDebt += (motion.rate || 0) * density * Math.max(0.6, area.width / (8 * area.em)) * seconds;
			for (; spawnDebt >= group; spawnDebt -= group) {
				const origin = { x: Math.random(), y: Math.random() };
				for (let i = 0; i < group && particles.length < PARTICLES_LIMIT; i++) particles.push(spawn(motion, i, origin));
			}
		}

		particles.forEach((particle) => {
			const startX = particle.x;
			particle.age += seconds;
			particle.x += particle.vx * seconds;
			particle.y += particle.vy * seconds;
			particle.angle += particle.spin * seconds;
			motion?.update?.(particle, seconds, area);

			// Skips the first step, where a motion that places its particles jumps them from the origin
			if (seconds > 0 && particle.age > seconds) particle.driftX += ((particle.x - startX) / seconds - particle.driftX) * Math.min(1, seconds * 6);
		});
		particles = particles.filter((particle) => particle.age < particle.life);
	};

	const draw = () => {
		if (!motion || !backContext || !frontContext) return;

		backContext.clearRect(0, 0, back.clientWidth, back.clientHeight);
		frontContext.clearRect(0, 0, front.clientWidth, front.clientHeight);
		particles.forEach((particle) => {
			const context = particle.front ? frontContext : backContext;
			const [scale, opacity] = motion?.envelope(particle) || [1, 1];
			const drawAt = (/** @type {number} */ x, /** @type {number} */ y, /** @type {number} */ fade, /** @type {number} */ shrink) => {
				context.save();
				context.translate(x, y);
				context.rotate(particle.angle);
				context.globalAlpha = opacity * fade;
				particle.shape.draw(context, particle, Math.max(0, particle.size * scale * shrink), area);
				context.restore();
			};

			// A trail fades and shrinks behind the particle, oldest first so the newer copies cover it
			const trail = motion?.trail?.(particle, area) || [];
			trail.toReversed().forEach((position, i) => {
				const nearness = (i + 1) / (trail.length + 1);
				drawAt(position.x, position.y, nearness * 0.5, 0.6 + nearness * 0.4);
			});
			drawAt(particle.x, particle.y, 1, 1);
		});
	};

	const changed = () => {
		configure();
		notify();
	};
	new MutationObserver(changed).observe(host, { attributes: true, attributeFilter: ["data-name-particle-shapes", "data-name-particle-motion", "style"] });
	new ResizeObserver(changed).observe(host);
	new IntersectionObserver(([entry]) => {
		onScreen = entry?.isIntersecting || false;
		notify();
	}).observe(host);
	configure();

	return {
		step,
		draw,
		active: () => onScreen && motion !== undefined,
		// A moment from partway through, drawn once
		still: () => {
			particles = [];
			spawnDebt = 0;
			for (let i = 0; i < PARTICLES_STILL_FRAME_SECONDS * 30; i++) step(1 / 30);
			draw();
		},
		onChange: (/** @type {() => void} */ callback) => {
			notify = callback;
			callback();
		},
	};
}

/**
 * @param {ParticleShape} shape
 * @param {Partial<Particle>} fields
 * @returns {Particle}
 */
function particle(shape, fields) {
	return {
		x: 0,
		y: 0,
		vx: 0,
		vy: 0,
		phase: 0,
		phaseSpeed: 0,
		age: 0,
		life: 1,
		size: 1,
		angle: 0,
		spin: 0,
		seed: Math.random() * 1000,
		slot: 0,
		driftX: 0,
		color: "",
		front: false,
		shape,
		...fields,
	};
}

/** @param {Particle} particle */
function lifeFraction(particle) {
	return particle.age / particle.life;
}

/**
 * @param {number} min
 * @param {number} max
 */
function random(min, max) {
	return min + Math.random() * (max - min);
}

/**
 * @param {CanvasRenderingContext2D} context
 * @param {number} x
 * @param {number} y
 * @param {number} radius
 */
function circle(context, x, y, radius) {
	context.beginPath();
	context.arc(x, y, Math.max(0, radius), 0, Math.PI * 2);
	context.fill();
}

/**
 * @param {number} start
 * @param {number} startHandle
 * @param {number} endHandle
 * @param {number} end
 * @param {number} t
 */
function cubicBezier(start, startHandle, endHandle, end, t) {
	const remaining = 1 - t;
	return remaining ** 3 * start + 3 * remaining ** 2 * t * startHandle + 3 * remaining * t ** 2 * endHandle + t ** 3 * end;
}

/**
 * Squashes the drawing across its upright axis to turn it all the way over, lingering near full width and sweeping quickly through edge-on.
 * @param {CanvasRenderingContext2D} context
 * @param {Particle} particle
 */
function turnOver(context, particle) {
	const turn = Math.cos(particle.age * 2.5 + particle.seed);
	context.scale(Math.sign(turn) * Math.abs(turn) ** 0.3, 1);
}

/**
 * A flattened circle tilted by which of the orbit planes it is, then stretched to wrap the name's box.
 * @param {number} phase
 * @param {number} plane
 * @param {ParticleArea} area
 */
function orbitPosition(phase, plane, area) {
	const tilt = (plane - 1) * 0.55;
	const across = Math.cos(phase);
	const depth = Math.sin(phase) * 0.32;
	const radiusX = area.width / 2 + 0.45 * area.em;
	const radiusY = area.height / 2 + 0.45 * area.em;

	return {
		x: area.left + area.width / 2 + (across * Math.cos(tilt) - depth * Math.sin(tilt)) * radiusX,
		y: area.top + area.height / 2 + (across * Math.sin(tilt) + depth * Math.cos(tilt)) * radiusY,
	};
}

/**
 * The way clockwise around the name's box, grown by a margin with its corners rounded, from the start of its top edge.
 * @param {number} margin
 * @param {number} rounding
 * @param {ParticleArea} area
 */
function encirclingPath(margin, rounding, area) {
	const left = area.left - margin;
	const top = area.top - margin;
	const right = area.left + area.width + margin;
	const bottom = area.top + area.height + margin;
	const corner = Math.min(rounding, (right - left) / 2, (bottom - top) / 2);
	const across = right - left - corner * 2;
	const down = bottom - top - corner * 2;
	const turn = (Math.PI / 2) * corner;

	// Each corner turns a quarter of the way around a circle set in from it
	const around = (/** @type {number} */ x, /** @type {number} */ y, /** @type {number} */ startAngle) => (/** @type {number} */ distance) => ({
		x: x + Math.cos(startAngle + distance / corner) * corner,
		y: y + Math.sin(startAngle + distance / corner) * corner,
	});

	/** @type {[number, (distance: number) => { x: number, y: number }][]} */
	const stretches = [
		[across, (distance) => ({ x: left + corner + distance, y: top })],
		[turn, around(right - corner, top + corner, -Math.PI / 2)],
		[down, (distance) => ({ x: right, y: top + corner + distance })],
		[turn, around(right - corner, bottom - corner, 0)],
		[across, (distance) => ({ x: right - corner - distance, y: bottom })],
		[turn, around(left + corner, bottom - corner, Math.PI / 2)],
		[down, (distance) => ({ x: left, y: bottom - corner - distance })],
		[turn, around(left + corner, top + corner, Math.PI)],
	];
	const length = stretches.reduce((sum, [stretch]) => sum + stretch, 0);

	return {
		length,
		// The point a fraction of the way around
		at: (/** @type {number} */ fraction) => {
			let distance = (((fraction % 1) + 1) % 1) * length;
			for (let i = 0; i < stretches.length - 1; i++) {
				const [stretch, point] = stretches[i] || [0, () => ({ x: left, y: top })];
				if (distance <= stretch) return point(distance);
				distance -= stretch;
			}
			const [, last] = stretches[stretches.length - 1] || [0, () => ({ x: left, y: top })];
			return last(distance);
		},
	};
}

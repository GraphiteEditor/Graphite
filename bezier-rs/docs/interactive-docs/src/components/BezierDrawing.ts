import { WasmBezier } from "@/../wasm/pkg";
import { COLORS, drawBezier, drawPoint, getContextFromCanvas, getPointSizeByIndex } from "@/utils/drawing";
import { BezierCallback, BezierPoint, WasmBezierMutatorKey } from "@/utils/types";
import { WasmBezierInstance } from "@/utils/wasm-comm";

// Offset to increase selectable range, used to make points easier to grab
const FUDGE_FACTOR = 3;

class BezierDrawing {
	static indexToMutator: WasmBezierMutatorKey[] = ["set_start", "set_handle_start", "set_handle_end", "set_end"];

	points: BezierPoint[];

	canvas: HTMLCanvasElement;

	ctx: CanvasRenderingContext2D;

	dragIndex: number | null;

	bezier: WasmBezierInstance;

	callback: BezierCallback;

	options: Record<string, number>;

	createThroughPoints: boolean;

	constructor(bezier: WasmBezierInstance, callback: BezierCallback, options: Record<string, number>, createThroughPoints = false) {
		this.bezier = bezier;
		this.callback = callback;
		this.options = options;
		this.createThroughPoints = createThroughPoints;
		this.points = bezier
			.get_points()
			.map((p) => JSON.parse(p))
			.map((p, i, points) => ({
				x: p.x,
				y: p.y,
				r: getPointSizeByIndex(i, points.length),
				selected: false,
				mutator: BezierDrawing.indexToMutator[points.length === 3 && i > 1 ? i + 1 : i],
			}));

		if (this.createThroughPoints && this.points.length === 4) {
			// Use the first handler as the middle point
			this.points = [this.points[0], this.points[1], this.points[3]];
		}

		const canvas = document.createElement("canvas");
		if (canvas === null) {
			throw Error("Failed to create canvas");
		}
		this.canvas = canvas;

		this.canvas.style.border = "solid 1px black";
		this.canvas.width = 200;
		this.canvas.height = 200;

		this.ctx = getContextFromCanvas(this.canvas);

		this.dragIndex = null; // Index of the point being moved

		this.canvas.addEventListener("mousedown", this.mouseDownHandler.bind(this));
		this.canvas.addEventListener("mousemove", this.mouseMoveHandler.bind(this));
		this.canvas.addEventListener("mouseup", this.deselectPointHandler.bind(this));
		this.canvas.addEventListener("mouseout", this.deselectPointHandler.bind(this));
		this.updateBezier();
	}

	clearFigure(): void {
		this.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
	}

	mouseMoveHandler(evt: MouseEvent): void {
		const mx = evt.offsetX;
		const my = evt.offsetY;

		if (this.dragIndex !== null) {
			const selectableRange = getPointSizeByIndex(this.dragIndex, this.points.length);
			if (mx - selectableRange > 0 && my - selectableRange > 0 && mx + selectableRange < this.canvas.width && my + selectableRange < this.canvas.height) {
				const selectedPoint = this.points[this.dragIndex];
				selectedPoint.x = mx;
				selectedPoint.y = my;
				this.bezier[selectedPoint.mutator](selectedPoint.x, selectedPoint.y);
				this.clearFigure();
			}
		}
		this.updateBezier();
	}

	mouseDownHandler(evt: MouseEvent): void {
		const mx = evt.offsetX;
		const my = evt.offsetY;
		for (let i = 0; i < this.points.length; i += 1) {
			const selectableRange = getPointSizeByIndex(i, this.points.length) + FUDGE_FACTOR;
			if (Math.abs(mx - this.points[i].x) < selectableRange && Math.abs(my - this.points[i].y) < selectableRange) {
				this.dragIndex = i;
				break;
			}
		}
		this.updateBezier();
	}

	deselectPointHandler(): void {
		if (this.dragIndex !== undefined) {
			this.clearFigure();
			this.dragIndex = null;
			this.updateBezier();
		}
	}

	updateBezier(options: Record<string, number> = {}): void {
		if (Object.values(options).length !== 0) {
			this.options = options;
		}
		this.clearFigure();

		const actualBezierPointLength = this.bezier.get_points().length;
		const pointsToDraw = this.createThroughPoints
			? (actualBezierPointLength === 3
					? WasmBezier.quadratic_through_points(
							this.points.map((p) => [p.x, p.y]),
							this.options.t
					  )
					: WasmBezier.cubic_through_points(
							this.points.map((p) => [p.x, p.y]),
							this.options.t,
							this.options.strut
					  )
			  )
					.get_points()
					.map((p) => JSON.parse(p))
			: this.points;

		drawBezier(this.ctx, pointsToDraw, this.dragIndex);
		if (this.createThroughPoints) {
			pointsToDraw.forEach((point, index) => {
				// Redraw on top of the the handler(s) to change the colour
				if (index !== 0 && index !== pointsToDraw.length - 1) {
					drawPoint(this.ctx, point, getPointSizeByIndex(index, pointsToDraw.length), COLORS.NON_INTERACTIVE.STROKE_1);
				}
			});
			// Draw the point that the curve was drawn through
			drawPoint(this.ctx, this.points[1], getPointSizeByIndex(1, this.points.length), this.dragIndex === 1 ? COLORS.INTERACTIVE.SELECTED : COLORS.INTERACTIVE.STROKE_1);
		}
		this.callback(this.canvas, this.bezier, this.options);
	}

	getCanvas(): HTMLCanvasElement {
		return this.canvas;
	}
}

export default BezierDrawing;

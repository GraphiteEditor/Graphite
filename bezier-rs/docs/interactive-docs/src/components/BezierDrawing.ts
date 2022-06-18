import { drawBezier, getContextFromCanvas } from "@/utils/drawing";
import { BezierCallback, BezierPoint, WasmBezierMutatorKey } from "@/utils/types";
import { WasmBezierInstance } from "@/utils/wasm-comm";

class BezierDrawing {
	static indexToMutator: WasmBezierMutatorKey[] = ["set_start", "set_handle_start", "set_handle_end", "set_end"];

	points: BezierPoint[];

	canvas: HTMLCanvasElement;

	ctx: CanvasRenderingContext2D;

	dragIndex: number | null;

	bezier: WasmBezierInstance;

	callback: BezierCallback;

	options: string;

	constructor(bezier: WasmBezierInstance, callback: BezierCallback, options: string) {
		this.bezier = bezier;
		this.callback = callback;
		this.options = options;
		this.points = bezier
			.get_points()
			.map((p) => JSON.parse(p))
			.map((p, i, points) => ({
				x: p.x,
				y: p.y,
				r: i === 0 || i === points.length - 1 ? 5 : 3,
				selected: false,
				mutator: BezierDrawing.indexToMutator[points.length === 3 && i > 1 ? i + 1 : i],
			}));

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

		if (this.dragIndex !== null && mx - 5 > 0 && my - 5 > 0 && mx + 5 < this.canvas.width && my + 5 < this.canvas.height) {
			const selectedPoint = this.points[this.dragIndex];
			selectedPoint.x = mx;
			selectedPoint.y = my;
			this.bezier[selectedPoint.mutator](selectedPoint.x, selectedPoint.y);
			this.clearFigure();
		}
		this.updateBezier();
	}

	mouseDownHandler(evt: MouseEvent): void {
		const mx = evt.offsetX;
		const my = evt.offsetY;
		for (let i = 0; i < this.points.length; i += 1) {
			if (
				Math.abs(mx - this.points[i].x) < 5 + 3 &&
				Math.abs(my - this.points[i].y) < 5 + 3 // Fudge factor makes the points easier to grab
			) {
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

	updateBezier(options = ""): void {
		if (options !== "") {
			this.options = options;
		}
		this.clearFigure();
		drawBezier(this.ctx, this.points, this.dragIndex);
		this.callback(this.canvas, this.bezier, this.options);
	}

	getCanvas(): HTMLCanvasElement {
		return this.canvas;
	}
}

export default BezierDrawing;

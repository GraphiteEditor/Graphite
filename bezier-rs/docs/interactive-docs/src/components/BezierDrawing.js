import { drawBezier } from "../utils/drawing";

class BezierDrawing {
	constructor(points, wasm) {
		this.wasm = wasm;
		this.points = points;

		this.canvas = document.createElement("canvas");
		this.canvas.width = 200;
		this.canvas.height = 200;

		this.ctx = this.canvas.getContext("2d");
		this.dragIndex = null; // Index of the point being moved

		this.canvas.addEventListener("mousedown", this.mouseDownHandler.bind(this));
		this.canvas.addEventListener("mousemove", this.mouseMoveHandler.bind(this));
		this.canvas.addEventListener("mouseup", this.deselectPointHandler.bind(this));
		this.canvas.addEventListener("mouseout", this.deselectPointHandler.bind(this));
		this.ctx.strokeRect(0, 0, this.canvas.width, this.canvas.height);

		this.updateBezier();
	}

	mouseMoveHandler(evt) {
		const mx = evt.offsetX;
		const my = evt.offsetY;

		if (
			this.dragIndex != null &&
			mx - this.points[this.dragIndex].r > 0 &&
			my - this.points[this.dragIndex].r > 0 &&
			mx + this.points[this.dragIndex].r < this.canvas.width &&
			my + this.points[this.dragIndex].r < this.canvas.height
		) {
			this.points[this.dragIndex].x = mx;
			this.points[this.dragIndex].y = my;
			this.ctx.clearRect(1, 1, this.canvas.width - 2, this.canvas.height - 2);
			this.updateBezier();
		}
	}

	mouseDownHandler(evt) {
		const mx = evt.offsetX;
		const my = evt.offsetY;
		for (let i = 0; i < this.points.length; i += 1) {
			if (
				Math.abs(mx - this.points[i].x) < this.points[i].r + 3 &&
				Math.abs(my - this.points[i].y) < this.points[i].r + 3 // Fudge factor makes the points easier to grab
			) {
				this.dragIndex = i;
				this.points[this.dragIndex].selected = true;
				break;
			}
		}
	}

	deselectPointHandler() {
		if (this.dragIndex != null) {
			this.points[this.dragIndex].selected = false;
			this.ctx.clearRect(1, 1, this.canvas.width - 2, this.canvas.height - 2);
			this.updateBezier();
			this.dragIndex = null;
		}
	}

	updateBezier() {
		if (this.points.length === 4) {
			this.bezier = this.wasm.WasmBezier.new_cubic(
				this.points[0].x,
				this.points[0].y,
				this.points[1].x,
				this.points[1].y,
				this.points[2].x,
				this.points[2].y,
				this.points[3].x,
				this.points[3].y
			);
		} else {
			this.bezier = this.wasm.WasmBezier.new_quad(this.points[0].x, this.points[0].y, this.points[1].x, this.points[1].y, this.points[2].x, this.points[2].y);
		}
		drawBezier(this.ctx, this.points);
	}

	getCanvas() {
		return this.canvas;
	}
}

export default BezierDrawing;

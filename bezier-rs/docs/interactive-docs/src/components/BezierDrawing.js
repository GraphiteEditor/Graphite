const drawLine = (ctx, p1, p2) => {
	ctx.strokeStyle = "grey";
	ctx.lineWidth = 1;

	ctx.beginPath();
	ctx.moveTo(p1.x, p1.y);
	ctx.lineTo(p2.x, p2.y);
	ctx.stroke();
};

const drawPoint = (ctx, p) => {
	// Outline the point
	ctx.strokeStyle = p.selected ? "blue" : "black";
	ctx.lineWidth = p.r / 3;
	ctx.beginPath();
	ctx.arc(p.x, p.y, p.r, 0, 2 * Math.PI, false);
	ctx.stroke();

	// Fill the point (hiding any overlapping lines)
	ctx.fillStyle = "white";
	ctx.beginPath();
	ctx.arc(p.x, p.y, p.r * (2 / 3), 0, 2 * Math.PI, false);
	ctx.fill();
};

const drawBezier = (ctx, points) => {
	/* Until a bezier representation is finalized, treat the points as follows
		points[0] = left endpoint
		points[1] = handle 1
		points[2] = (optional) handle 2
		points[3] = right endpoint
	*/
	const left = points[0];
	let right = null;
	let handle1 = null;
	let handle2 = null;
	if (points.length === 4) {
		handle1 = points[1];
		handle2 = points[2];
		right = points[3];
	} else {
		handle1 = points[1];
		handle2 = handle1;
		right = points[2];
	}

	ctx.strokeStyle = "black";
	ctx.lineWidth = 2;

	ctx.beginPath();
	ctx.moveTo(points[0].x, points[0].y);
	if (points.length === 3) {
		ctx.quadraticCurveTo(handle1.x, handle1.y, right.x, right.y);
	} else {
		ctx.bezierCurveTo(handle1.x, handle1.y, handle2.x, handle2.y, right.x, right.y);
	}
	ctx.stroke();

	drawLine(ctx, left, handle1);
	drawLine(ctx, right, handle2);

	points.forEach((point) => {
		drawPoint(ctx, point);
	});
};

class BezierDrawing {
	constructor(bezier) {
		this.canvas = document.createElement("canvas");
		this.canvas.width = 200;
		this.canvas.height = 200;

		this.points = bezier.get_points().map((point) => JSON.parse(point));
		this.points.forEach((point, idx) => {
			if (idx === 0 || idx === this.points.length - 1) {
				point.r = 5;
			} else {
				point.r = 3;
			}
		});
		this.ctx = this.canvas.getContext("2d");
		this.dragIndex = null; // Index of the point being moved

		this.canvas.addEventListener("mousedown", this.mouseDownHandler.bind(this));
		this.canvas.addEventListener("mousemove", this.mouseMoveHandler.bind(this));
		this.canvas.addEventListener("mouseup", this.deselectPointHandler.bind(this));
		this.canvas.addEventListener("mouseout", this.deselectPointHandler.bind(this));
		this.ctx.strokeRect(0, 0, this.canvas.width, this.canvas.height);
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
			this.drawBezier();
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
			this.drawBezier();
			this.dragIndex = null;
		}
	}

	drawBezier() {
		drawBezier(this.ctx, this.points);
	}

	getCanvas() {
		return this.canvas;
	}
}
export { BezierDrawing };

export const drawLine = (ctx, p1, p2) => {
	ctx.strokeStyle = "grey";
	ctx.lineWidth = 1;

	ctx.beginPath();
	ctx.moveTo(p1.x, p1.y);
	ctx.lineTo(p2.x, p2.y);
	ctx.stroke();
};

export const drawPoint = (ctx, p) => {
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

export const drawBezier = (ctx, points) => {
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

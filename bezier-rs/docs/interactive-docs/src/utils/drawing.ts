import { Point, WasmBezierInstance } from "@/utils/types";

export const getPointSizeByIndex = (index: number, numPoints: number, radius = 5): number => (index === 0 || index === numPoints - 1 ? radius : (radius * 2) / 3);

export const getContextFromCanvas = (canvas: HTMLCanvasElement): CanvasRenderingContext2D => {
	const ctx = canvas.getContext("2d");
	if (ctx === null) {
		throw Error("Failed to fetch context");
	}
	return ctx;
};

export const drawLine = (ctx: CanvasRenderingContext2D, point1: Point, point2: Point, strokeColor = "gray"): void => {
	ctx.strokeStyle = strokeColor;
	ctx.lineWidth = 1;

	ctx.beginPath();
	ctx.moveTo(point1.x, point1.y);
	ctx.lineTo(point2.x, point2.y);
	ctx.stroke();
};

export const drawPoint = (ctx: CanvasRenderingContext2D, point: Point, radius: number, strokeColor = "black"): void => {
	// Outline the point
	ctx.strokeStyle = strokeColor;
	ctx.lineWidth = radius / 3;
	ctx.beginPath();
	ctx.arc(point.x, point.y, radius, 0, 2 * Math.PI, false);
	ctx.stroke();

	// Fill the point (hiding any overlapping lines)
	ctx.fillStyle = "white";
	ctx.beginPath();
	ctx.arc(point.x, point.y, radius * (2 / 3), 0, 2 * Math.PI, false);
	ctx.fill();
};

export const drawText = (ctx: CanvasRenderingContext2D, text: string, x: number, y: number): void => {
	ctx.fillStyle = "black";
	ctx.font = "16px Arial";
	ctx.fillText(text, x, y);
};

export const drawBezierHelper = (ctx: CanvasRenderingContext2D, bezier: WasmBezierInstance, stroke = "black", radius = 5): void => {
	drawBezier(
		ctx,
		bezier.get_points().map((p) => JSON.parse(p)),
		stroke,
		null,
		radius
	);
};

export const drawBezier = (ctx: CanvasRenderingContext2D, points: Point[], stroke = "black", dragIndex: number | null = null, radius = 5): void => {
	/* Until a bezier representation is finalized, treat the points as follows
		points[0] = start point
		points[1] = handle start
		points[2] = (optional) handle end
		points[3] = end point
	*/
	const start = points[0];
	let end = null;
	let handleStart = null;
	let handleEnd = null;
	if (points.length === 4) {
		handleStart = points[1];
		handleEnd = points[2];
		end = points[3];
	} else {
		handleStart = points[1];
		handleEnd = handleStart;
		end = points[2];
	}

	ctx.strokeStyle = stroke;
	ctx.lineWidth = 2;

	ctx.beginPath();
	ctx.moveTo(points[0].x, points[0].y);
	if (points.length === 3) {
		ctx.quadraticCurveTo(handleStart.x, handleStart.y, end.x, end.y);
	} else {
		ctx.bezierCurveTo(handleStart.x, handleStart.y, handleEnd.x, handleEnd.y, end.x, end.y);
	}
	ctx.stroke();

	drawLine(ctx, start, handleStart, stroke);
	drawLine(ctx, end, handleEnd, stroke);

	points.forEach((point, index) => {
		drawPoint(ctx, point, getPointSizeByIndex(index, points.length, radius), index === dragIndex ? "Blue" : stroke);
	});
};

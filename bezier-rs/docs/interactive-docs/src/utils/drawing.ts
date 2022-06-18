import { Point } from "@/utils/types";

export const getContextFromCanvas = (canvas: HTMLCanvasElement): CanvasRenderingContext2D => {
	const ctx = canvas.getContext("2d");
	if (ctx === null) {
		throw Error("Failed to fetch context");
	}
	return ctx;
};

export const drawLine = (ctx: CanvasRenderingContext2D, p1: Point, p2: Point, stroke = "grey"): void => {
	ctx.strokeStyle = stroke;
	ctx.lineWidth = 1;

	ctx.beginPath();
	ctx.moveTo(p1.x, p1.y);
	ctx.lineTo(p2.x, p2.y);
	ctx.stroke();
};

export const drawPoint = (ctx: CanvasRenderingContext2D, p: Point, r: number, stroke = "black"): void => {
	// Outline the point
	ctx.strokeStyle = stroke;
	ctx.lineWidth = r / 3;
	ctx.beginPath();
	ctx.arc(p.x, p.y, r, 0, 2 * Math.PI, false);
	ctx.stroke();

	// Fill the point (hiding any overlapping lines)
	ctx.fillStyle = "white";
	ctx.beginPath();
	ctx.arc(p.x, p.y, r * (2 / 3), 0, 2 * Math.PI, false);
	ctx.fill();
};

export const drawText = (ctx: CanvasRenderingContext2D, text: string, x: number, y: number): void => {
	ctx.fillStyle = "black";
	ctx.font = "16px Arial";
	ctx.fillText(text, x, y);
};

export const drawBezier = (ctx: CanvasRenderingContext2D, points: Point[], dragIndex: number | null = null): void => {
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

	ctx.strokeStyle = "black";
	ctx.lineWidth = 2;

	ctx.beginPath();
	ctx.moveTo(points[0].x, points[0].y);
	if (points.length === 3) {
		ctx.quadraticCurveTo(handleStart.x, handleStart.y, end.x, end.y);
	} else {
		ctx.bezierCurveTo(handleStart.x, handleStart.y, handleEnd.x, handleEnd.y, end.x, end.y);
	}
	ctx.stroke();

	drawLine(ctx, start, handleStart);
	drawLine(ctx, end, handleEnd);

	points.forEach((point, index) => {
		drawPoint(ctx, point, index === 0 || index === points.length - 1 ? 5 : 3, index === dragIndex ? "Blue" : "Black");
	});
};

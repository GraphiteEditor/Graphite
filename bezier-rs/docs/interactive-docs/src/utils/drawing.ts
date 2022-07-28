import { BezierStyleConfig, CircleSector, Point, WasmBezierInstance } from "@/utils/types";

const HANDLE_RADIUS_FACTOR = 2 / 3;
const DEFAULT_ENDPOINT_RADIUS = 5;

export const COLORS = {
	CANVAS: "white",
	INTERACTIVE: {
		STROKE_1: "black",
		STROKE_2: "gray",
		SELECTED: "blue",
	},
	NON_INTERACTIVE: {
		STROKE_1: "red",
		STROKE_2: "orange",
	},
};

export const isIndexFirstOrLast = (index: number, arrayLength: number): boolean => index === 0 || index === arrayLength - 1;

export const getPointSizeByIndex = (index: number, numPoints: number, radius = DEFAULT_ENDPOINT_RADIUS): number => (isIndexFirstOrLast(index, numPoints) ? radius : radius * HANDLE_RADIUS_FACTOR);

export const getContextFromCanvas = (canvas: HTMLCanvasElement): CanvasRenderingContext2D => {
	const ctx = canvas.getContext("2d");
	if (ctx === null) {
		throw Error("Failed to fetch context");
	}
	return ctx;
};

export const drawLine = (ctx: CanvasRenderingContext2D, point1: Point, point2: Point, strokeColor = COLORS.INTERACTIVE.STROKE_2, lineWidth = 1): void => {
	ctx.strokeStyle = strokeColor;
	ctx.lineWidth = lineWidth;

	ctx.beginPath();
	ctx.moveTo(point1.x, point1.y);
	ctx.lineTo(point2.x, point2.y);
	ctx.stroke();
};

export const drawPoint = (ctx: CanvasRenderingContext2D, point: Point, radius: number, strokeColor = COLORS.INTERACTIVE.STROKE_1): void => {
	// Outline the point
	ctx.strokeStyle = strokeColor;
	ctx.lineWidth = radius / 3;
	ctx.beginPath();
	ctx.arc(point.x, point.y, radius, 0, 2 * Math.PI, false);
	ctx.stroke();

	// Fill the point (hiding any overlapping lines)
	ctx.fillStyle = COLORS.CANVAS;
	ctx.beginPath();
	ctx.arc(point.x, point.y, radius * HANDLE_RADIUS_FACTOR, 0, 2 * Math.PI, false);
	ctx.fill();
};

export const drawText = (ctx: CanvasRenderingContext2D, text: string, x: number, y: number, textColor = COLORS.INTERACTIVE.STROKE_1): void => {
	ctx.fillStyle = textColor;
	ctx.font = "16px Arial";
	ctx.fillText(text, x, y);
};

export const drawCurve = (ctx: CanvasRenderingContext2D, points: Point[], strokeColor = COLORS.INTERACTIVE.STROKE_1): void => {
	ctx.strokeStyle = strokeColor;
	ctx.lineWidth = 2;

	ctx.beginPath();
	ctx.moveTo(points[0].x, points[0].y);
	if (points.length === 3) {
		ctx.quadraticCurveTo(points[1].x, points[1].y, points[2].x, points[2].y);
	} else {
		ctx.bezierCurveTo(points[1].x, points[1].y, points[2].x, points[2].y, points[3].x, points[3].y);
	}
	ctx.stroke();
};

export const drawCircleSector = (ctx: CanvasRenderingContext2D, circleSector: CircleSector, strokeColor = COLORS.INTERACTIVE.STROKE_1, fillColor = COLORS.NON_INTERACTIVE.STROKE_1): void => {
	ctx.strokeStyle = strokeColor;
	ctx.fillStyle = fillColor;
	ctx.lineWidth = 2;

	const { center, radius, start_angle: startAngle, end_angle: endAngle } = circleSector;
	ctx.beginPath();
	ctx.moveTo(center.x, center.y);
	ctx.arc(center.x, center.y, radius, startAngle, endAngle);
	ctx.lineTo(center.x, center.y);
	ctx.closePath();
	ctx.fill();
};

export const drawBezierHelper = (ctx: CanvasRenderingContext2D, bezier: WasmBezierInstance, bezierStyleConfig: Partial<BezierStyleConfig> = {}): void => {
	const points = JSON.parse(bezier.get_points());
	drawBezier(ctx, points, null, bezierStyleConfig);
};

export const drawBezier = (ctx: CanvasRenderingContext2D, points: Point[], dragIndex: number | null = null, bezierStyleConfig: Partial<BezierStyleConfig> = {}): void => {
	const styleConfig: BezierStyleConfig = {
		curveStrokeColor: COLORS.INTERACTIVE.STROKE_1,
		handleStrokeColor: COLORS.INTERACTIVE.STROKE_1,
		handleLineStrokeColor: COLORS.INTERACTIVE.STROKE_1,
		radius: DEFAULT_ENDPOINT_RADIUS,
		drawHandles: true,
		...bezierStyleConfig,
	};
	// If the handle or handle line colors are not specified, use the same color as the rest of the curve
	if (bezierStyleConfig.curveStrokeColor) {
		if (!bezierStyleConfig.handleStrokeColor) {
			styleConfig.handleStrokeColor = bezierStyleConfig.curveStrokeColor;
		}
		if (!bezierStyleConfig.handleLineStrokeColor) {
			styleConfig.handleLineStrokeColor = bezierStyleConfig.curveStrokeColor;
		}
	}
	// Points passed to drawBezier are interpreted as follows
	//	points[0] = start point
	//	points[1] = handle start
	//	points[2] = (optional) handle end
	//	points[3] = end point
	const start = points[0];
	let end = null;
	let handleStart = null;
	let handleEnd = null;
	if (points.length === 4) {
		handleStart = points[1];
		handleEnd = points[2];
		end = points[3];
	} else if (points.length === 3) {
		handleStart = points[1];
		handleEnd = handleStart;
		end = points[2];
	} else {
		handleStart = start;
		handleEnd = points[1];
		end = handleEnd;
	}

	if (points.length === 2) {
		drawLine(ctx, start, end, styleConfig.curveStrokeColor, 2);
	} else {
		drawCurve(ctx, points, styleConfig.curveStrokeColor);
		if (styleConfig.drawHandles) {
			drawLine(ctx, start, handleStart, styleConfig.handleLineStrokeColor);
			drawLine(ctx, end, handleEnd, styleConfig.handleLineStrokeColor);
		}
	}

	points.forEach((point, index) => {
		if (styleConfig.drawHandles || isIndexFirstOrLast(index, points.length)) {
			const strokeColor = isIndexFirstOrLast(index, points.length) ? styleConfig.curveStrokeColor : styleConfig.handleStrokeColor;
			drawPoint(ctx, point, getPointSizeByIndex(index, points.length, styleConfig.radius), index === dragIndex ? COLORS.INTERACTIVE.SELECTED : strokeColor);
		}
	});
};

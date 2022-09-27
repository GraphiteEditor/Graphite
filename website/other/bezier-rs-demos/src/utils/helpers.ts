import { BezierCurveType, WasmBezierConstructorKey } from "@/utils/types";

export const getCurveType = (numPoints: number): BezierCurveType => {
	switch (numPoints) {
		case 2:
			return BezierCurveType.Linear;
		case 3:
			return BezierCurveType.Quadratic;
		case 4:
			return BezierCurveType.Cubic;
		default:
			throw new Error("Invalid number of points for a bezier");
	}
};

export const getConstructorKey = (bezierCurveType: BezierCurveType): WasmBezierConstructorKey => {
	switch (bezierCurveType) {
		case BezierCurveType.Linear:
			return "new_linear";
		case BezierCurveType.Quadratic:
			return "new_quadratic";
		case BezierCurveType.Cubic:
			return "new_cubic";
		default:
			throw new Error("Invalid value for a BezierCurveType");
	}
};

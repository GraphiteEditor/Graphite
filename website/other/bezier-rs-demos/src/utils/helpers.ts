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
			// Not possible
			return BezierCurveType.Linear;
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
			// Not possible
			return "new_linear";
	}
};

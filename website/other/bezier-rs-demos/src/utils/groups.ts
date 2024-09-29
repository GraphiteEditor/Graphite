import { newBezierDemo } from "@/components/BezierDemo";
import { newSubpathDemo } from "@/components/SubpathDemo";
import type { BezierFeatureKey, BezierFeatureOptions } from "@/features/bezier-features";
import bezierFeatures from "@/features/bezier-features";
import type { SubpathFeatureKey, SubpathFeatureOptions } from "@/features/subpath-features";
import subpathFeatures from "@/features/subpath-features";
import { renderDemoGroup } from "@/utils/render";
import { BEZIER_CURVE_TYPE, getBezierDemoPointDefaults } from "@/utils/types";
import type { BezierCurveType, BezierDemoArgs, SubpathDemoArgs } from "@/utils/types";

export function bezierDemoGroup(key: BezierFeatureKey, options: BezierFeatureOptions): HTMLDivElement {
	const demoOptions = options.demoOptions || {};
	const triggerOnMouseMove = options.triggerOnMouseMove || false;
	const name = bezierFeatures[key].name;
	const id = `bezier/${key}`;

	const demos: BezierDemoArgs[] = BEZIER_CURVE_TYPE.map((curveType: BezierCurveType) => ({
		title: curveType,
		disabled: demoOptions[curveType]?.disabled || false,
		points: demoOptions[curveType]?.customPoints || getBezierDemoPointDefaults()[curveType],
		inputOptions: demoOptions[curveType]?.inputOptions || demoOptions.Quadratic?.inputOptions || [],
	}));

	return renderDemoGroup(id, name, demos, (demo: BezierDemoArgs): HTMLElement => newBezierDemo(demo.title, demo.points, key, demo.inputOptions, triggerOnMouseMove).element);
}

export function subpathDemoGroup(key: SubpathFeatureKey, options: SubpathFeatureOptions): HTMLDivElement {
	const inputOptions = options.inputOptions || [];
	const triggerOnMouseMove = options.triggerOnMouseMove || false;
	const name = subpathFeatures[key].name;
	const id = `subpath/${key}`;

	const demos: SubpathDemoArgs[] = [
		{
			title: "Open Subpath",
			triples: [
				[[45, 20], undefined, [35, 90]],
				[[175, 40], [85, 40], undefined],
				[[200, 175], undefined, undefined],
				[[125, 100], [65, 120], undefined],
			],
			closed: false,
		},
		{
			title: "Closed Subpath",
			triples: [
				[[60, 125], undefined, [65, 40]],
				[[155, 30], [145, 120], undefined],
				[
					[170, 150],
					[200, 90],
					[95, 185],
				],
			],
			closed: true,
		},
	];

	const buildDemo = (demo: SubpathDemoArgs): HTMLElement => {
		const newInputOptions = inputOptions.map((option) => ({
			...option,
			disabled: option.isDisabledForClosed && demo.closed,
		}));
		return newSubpathDemo(demo.title, demo.triples, key, demo.closed, newInputOptions, triggerOnMouseMove).element;
	};

	return renderDemoGroup(id, name, demos, buildDemo);
}

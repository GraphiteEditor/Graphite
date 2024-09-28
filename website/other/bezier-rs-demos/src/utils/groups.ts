import type { BezierFeatureKey, BezierFeatureOptions } from "@/features/bezier-features";
import bezierFeatures from "@/features/bezier-features";
import type { SubpathFeatureKey, SubpathFeatureOptions } from "@/features/subpath-features";
import subpathFeatures from "@/features/subpath-features";
import { renderDemoGroup } from "@/utils/render";
import { BEZIER_CURVE_TYPE, BEZIER_DEMO_DEFAULTS } from "@/utils/types";
import type { BezierCurveType, BezierDemoArgs, SubpathDemoArgs } from "@/utils/types";

export function bezierDemoGroup(key: BezierFeatureKey, options: BezierFeatureOptions): HTMLDivElement {
	const element = document.createElement("div");

	const demoOptions = options.demoOptions || {};
	const triggerOnMouseMove = options.triggerOnMouseMove || false;
	const name = bezierFeatures[key].name;
	const id = `bezier/${key}`;

	const demos: BezierDemoArgs[] = BEZIER_CURVE_TYPE.map((curveType: BezierCurveType) => ({
		title: curveType,
		disabled: demoOptions[curveType]?.disabled || false,
		points: demoOptions[curveType]?.customPoints || BEZIER_DEMO_DEFAULTS[curveType],
		inputOptions: demoOptions[curveType]?.inputOptions || demoOptions.Quadratic?.inputOptions || [],
	}));

	const buildDemo = (demo: BezierDemoArgs): HTMLElement => {
		const bezierDemo = document.createElement("bezier-demo");
		bezierDemo.setAttribute("title", demo.title);
		bezierDemo.setAttribute("points", JSON.stringify(demo.points));
		bezierDemo.setAttribute("key", key);
		bezierDemo.setAttribute("inputOptions", JSON.stringify(demo.inputOptions));
		bezierDemo.setAttribute("triggerOnMouseMove", String(triggerOnMouseMove));
		return bezierDemo;
	};

	renderDemoGroup(element, id, name, demos, buildDemo);

	return element;
}

export function subpathDemoGroup(key: SubpathFeatureKey, options: SubpathFeatureOptions): HTMLDivElement {
	const element = document.createElement("div");

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
		const subpathDemo = document.createElement("subpath-demo");
		subpathDemo.setAttribute("title", demo.title);
		subpathDemo.setAttribute("triples", JSON.stringify(demo.triples));
		subpathDemo.setAttribute("closed", String(demo.closed));
		subpathDemo.setAttribute("key", key);

		const newInputOptions = inputOptions.map((option) => ({
			...option,
			disabled: option.isDisabledForClosed && demo.closed,
		}));
		subpathDemo.setAttribute("inputOptions", JSON.stringify(newInputOptions));
		subpathDemo.setAttribute("triggerOnMouseMove", String(triggerOnMouseMove));
		return subpathDemo;
	};

	renderDemoGroup(element, id, name, demos, buildDemo);

	return element;
}

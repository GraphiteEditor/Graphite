import { tSliderOptions } from "@/utils/options";
import { ComputeType, WasmSubpathInstance } from "@/utils/types";

const subpathFeatures = {
  "Constructor": {
    callback: (subpath: WasmSubpathInstance): string => subpath.to_svg(),
  },
  "Length": {
    callback: (subpath: WasmSubpathInstance): string => subpath.length(),
  },
  "Evaluate": {
    callback: (subpath: WasmSubpathInstance, options: Record<string, number>, _: undefined, computeType: ComputeType): string => subpath.evaluate(options.computeArgument, computeType),
    sliderOptions: [{ ...tSliderOptions, variable: "computeArgument" }],
    chooseComputeType: true,
  },
  "Project": {
    callback: (subpath: WasmSubpathInstance, _: Record<string, number>, mouseLocation?: [number, number]): string =>
      mouseLocation ? subpath.project(mouseLocation[0], mouseLocation[1]) : subpath.to_svg(),
    triggerOnMouseMove: true,
  },
  "Intersect (Line Segment)": {
    callback: (subpath: WasmSubpathInstance): string =>
      subpath.intersect_line_segment([
        [150, 150],
        [20, 20],
      ]),
  },
  "Intersect (Quadratic segment)": {
    callback: (subpath: WasmSubpathInstance): string =>
      subpath.intersect_quadratic_segment([
        [20, 80],
        [180, 10],
        [90, 120],
      ]),
  },
  "Intersect (Cubic segment)": {
    callback: (subpath: WasmSubpathInstance): string =>
      subpath.intersect_cubic_segment([
        [40, 20],
        [100, 40],
        [40, 120],
        [175, 140],
      ]),
  },
};

export default subpathFeatures;

import paper from "paper/dist/paper-core";

// Required setup to be used headlessly
paper.setup(new paper.Size(1, 1));
paper.view.autoUpdate = false;

export function booleanUnion(path1: string, path2: string): string {
	return booleanOperation(path1, path2, "unite");
}

export function booleanSubtract(path1: string, path2: string): string {
	return booleanOperation(path1, path2, "subtract");
}

export function booleanIntersect(path1: string, path2: string): string {
	return booleanOperation(path1, path2, "intersect");
}

export function booleanDifference(path1: string, path2: string): string {
	return booleanOperation(path1, path2, "exclude");
}

export function booleanDivide(path1: string, path2: string): string {
	return booleanOperation(path1, path2, "intersect") + booleanOperation(path1, path2, "exclude");
}

function booleanOperation(path1: string, path2: string, operation: "unite" | "subtract" | "intersect" | "exclude"): string {
	const paperPath1 = new paper.Path(path1);
	const paperPath2 = new paper.Path(path2);
	const result = paperPath1[operation](paperPath2);
	paperPath1.remove();
	paperPath2.remove();
	return result.pathData;
}

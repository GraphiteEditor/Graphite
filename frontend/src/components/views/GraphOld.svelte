<script lang="ts">
	import { getContext } from "svelte";
	import { cubicInOut } from "svelte/easing";
	import { fade } from "svelte/transition";

	import type { Editor } from "@graphite/editor";
	import { type FrontendGraphInputOld, type FrontendGraphOutputOld, type FrontendGraphInputNew, type FrontendGraphOutputNew } from "@graphite/messages";
	import type { NodeGraphState } from "@graphite/state-providers/node-graph";
	import type { IconName } from "@graphite/utility-functions/icons";

	import NodeCatalog from "@graphite/components/floating-menus/NodeCatalog.svelte";
	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import IconButton from "@graphite/components/widgets/buttons/IconButton.svelte";
	import TextButton from "@graphite/components/widgets/buttons/TextButton.svelte";
	import RadioInput from "@graphite/components/widgets/inputs/RadioInput.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";
	import Separator from "@graphite/components/widgets/labels/Separator.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";

	const GRID_COLLAPSE_SPACING = 10;
	const GRID_SIZE = 24;

	const editor = getContext<Editor>("editor");
	const nodeGraph = getContext<NodeGraphState>("nodeGraph");

	let graph: HTMLDivElement | undefined;

	$: gridSpacing = calculateGridSpacing($nodeGraph.transformOld.scale);
	$: gridDotRadius = 1 + Math.floor($nodeGraph.transformOld.scale - 0.5 + 0.001) / 2;

	function nodeIcon(icon?: string): IconName {
		if (!icon) return "NodeNodes";
		const iconMap: Record<string, IconName> = {
			Output: "NodeOutput",
		};
		return iconMap[icon] || "NodeNodes";
	}
</script>

<script lang="ts">
	import { createEventDispatcher, getContext, onMount } from "svelte";
	import { SvelteMap } from "svelte/reactivity";

	import type { FrontendNodeType } from "@graphite/messages";
	import type { NodeGraphState } from "@graphite/state-providers/node-graph";

	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import TextButton from "@graphite/components/widgets/buttons/TextButton.svelte";
	import TextInput from "@graphite/components/widgets/inputs/TextInput.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";

	const dispatch = createEventDispatcher<{ selectNodeType: string }>();
	const nodeGraph = getContext<NodeGraphState>("nodeGraph");

	// Content
	export let disabled = false;
	// Behavior
	export let initialSearchTerm = "";

	let nodeSearchInput: TextInput | undefined = undefined;
	let searchTerm = initialSearchTerm;

	$: nodeCategories = buildNodeCategories(searchTerm);

	type NodeCategoryDetails = {
		nodes: FrontendNodeType[];
		open: boolean;
	};

	function buildNodeCategories(searchTerm: string): [string, NodeCategoryDetails][] {
		const nodeTypes = $nodeGraph.nodeTypes;
		const categories = new SvelteMap<string, NodeCategoryDetails>();
		const isTypeSearch = searchTerm.toLowerCase().startsWith("type:");
		let typeSearchTerm = "";
		let remainingSearchTerms = [searchTerm.toLowerCase()];

		if (isTypeSearch) {
			// Extract the first word after "type:" as the type search
			const searchParts = searchTerm.substring(5).trim().split(/\s+/);
			typeSearchTerm = searchParts[0].toLowerCase();

			remainingSearchTerms = searchParts.slice(1).map((term) => term.toLowerCase());
		}

		nodeTypes.forEach((node) => {
			let matchesTypeSearch = true;
			let matchesRemainingTerms = true;

			if (isTypeSearch && typeSearchTerm) {
				matchesTypeSearch = node.inputTypes?.some((inputType) => inputType.toLowerCase().includes(typeSearchTerm)) || false;
			}

			if (remainingSearchTerms.length > 0) {
				matchesRemainingTerms = remainingSearchTerms.every((term) => {
					const nameMatch = node.name.toLowerCase().includes(term);
					const categoryMatch = node.category.toLowerCase().includes(term);

					// Quick and dirty hack to alias "Layer" to "Merge" in the search
					const layerAliasMatch = node.name === "Merge" && "layer".includes(term);

					return nameMatch || categoryMatch || layerAliasMatch;
				});
			}

			// Node matches if it passes both type search and remaining terms filters
			const includesSearchTerm = matchesTypeSearch && matchesRemainingTerms;

			if (searchTerm.length > 0 && !includesSearchTerm) {
				return;
			}

			const category = categories.get(node.category);
			let open = includesSearchTerm;
			if (searchTerm.length === 0) {
				open = false;
			}

			if (category) {
				category.open = category.open || open;
				category.nodes.push(node);
			} else {
				categories.set(node.category, {
					open,
					nodes: [node],
				});
			}
		});

		const START_CATEGORIES_ORDER = ["UNCATEGORIZED", "General", "Value", "Math", "Style"];
		const END_CATEGORIES_ORDER = ["Debug"];
		return Array.from(categories)
			.sort((a, b) => a[0].localeCompare(b[0]))
			.sort((a, b) => {
				const aIndex = START_CATEGORIES_ORDER.findIndex((x) => a[0].startsWith(x));
				const bIndex = START_CATEGORIES_ORDER.findIndex((x) => b[0].startsWith(x));
				if (aIndex !== -1 && bIndex !== -1) return aIndex - bIndex;
				if (aIndex !== -1) return -1;
				if (bIndex !== -1) return 1;
				return 0;
			})
			.sort((a, b) => {
				const aIndex = END_CATEGORIES_ORDER.findIndex((x) => a[0].startsWith(x));
				const bIndex = END_CATEGORIES_ORDER.findIndex((x) => b[0].startsWith(x));
				if (aIndex !== -1 && bIndex !== -1) return aIndex - bIndex;
				if (aIndex !== -1) return 1;
				if (bIndex !== -1) return -1;
				return 0;
			});
	}

	onMount(() => {
		setTimeout(() => nodeSearchInput?.focus(), 0);
	});
</script>

<LayoutCol class="node-catalog">
	<TextInput placeholder="Search Nodes…" value={searchTerm} on:value={({ detail }) => (searchTerm = detail)} bind:this={nodeSearchInput} />
	<div class="list-results" on:wheel|passive|stopPropagation>
		{#each nodeCategories as nodeCategory}
			<details open={nodeCategory[1].open}>
				<summary>
					<TextLabel>{nodeCategory[0]}</TextLabel>
				</summary>
				{#each nodeCategory[1].nodes as nodeType}
					<TextButton
						{disabled}
						label={nodeType.name}
						tooltipLabel={nodeType.name}
						tooltipDescription={nodeType.identifier ? $nodeGraph.nodeDescriptions.get(nodeType.identifier) : undefined}
						action={() => dispatch("selectNodeType", nodeType.identifier)}
					/>
				{/each}
			</details>
		{:else}
			<TextLabel>No search results</TextLabel>
		{/each}
	</div>
</LayoutCol>

<style lang="scss" global>
	.node-catalog {
		max-height: 30vh;
		min-width: 250px;

		.text-input {
			flex: 0 0 auto;
			margin-bottom: 4px;
		}

		.list-results {
			overflow-y: auto;
			flex: 1 1 auto;

			details {
				cursor: pointer;
				position: relative;
				margin-right: 4px;

				&[open] summary .text-label::before {
					transform: rotate(90deg);
				}

				summary {
					display: flex;

					.text-label {
						padding-left: 16px;
						position: relative;
						pointer-events: none;

						&::before {
							content: "";
							position: absolute;
							margin: auto;
							top: 0;
							bottom: 0;
							left: 0;
							width: 8px;
							height: 8px;
							background: var(--icon-expand-collapse-arrow);
						}
					}
				}

				.text-button {
					width: 100%;
					margin: 4px 0;
					text-align: left;
				}

				&:last-child .text-button {
					margin-bottom: 0;
				}
			}
		}
	}
</style>

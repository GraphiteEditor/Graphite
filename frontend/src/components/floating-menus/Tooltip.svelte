<script lang="ts">
	import { getContext } from "svelte";

	import type { Editor } from "@graphite/editor";
	import type { TooltipState } from "@graphite/state-providers/tooltip";

	import FloatingMenu from "@graphite/components/layout/FloatingMenu.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";

	const tooltip = getContext<TooltipState>("tooltip");
	const editor = getContext<Editor>("editor");

	let self: FloatingMenu | undefined;

	$: label = filterTodo($tooltip.element?.getAttribute("data-tooltip-label")?.trim());
	$: description = filterTodo($tooltip.element?.getAttribute("data-tooltip-description")?.trim());
	$: shortcut = filterTodo($tooltip.element?.getAttribute("data-tooltip-shortcut")?.trim());

	// TODO: Once all TODOs are replaced with real text, remove this function
	function filterTodo(text: string | undefined): string | undefined {
		if (text?.trim().toUpperCase() === "TODO" && !editor.handle.inDevelopmentMode()) return "";
		return text;
	}
</script>

<div class="tooltip" style:top={`${$tooltip.position.y}px`} style:left={`${$tooltip.position.x}px`}>
	{#if label || description}
		<FloatingMenu open={true} type="Tooltip" direction="Bottom" bind:this={self}>
			{#if label}
				<TextLabel class="tooltip-label">{label}</TextLabel>
			{/if}
			{#if description}
				<TextLabel class="tooltip-description">{description}</TextLabel>
			{/if}
			{#if shortcut}
				<TextLabel class="tooltip-shortcut">Shortcut: {shortcut}</TextLabel>
			{/if}
		</FloatingMenu>
	{/if}
</div>

<style lang="scss" global>
	.tooltip {
		position: absolute;
		pointer-events: none;
		width: 0;
		height: 0;

		.floating-menu-content {
			max-width: Min(400px, 50vw);

			.text-label {
				white-space: pre-wrap;

				+ .text-label {
					margin-top: 4px;
				}

				&.tooltip-description,
				&.tooltip-shortcut {
					color: var(--color-b-lightgray);
				}
			}
		}
	}
</style>

<script lang="ts">
	import { type IconName, type IconSize } from "~/src/utility-functions/icons";

	import IconLabel from "~/src/components/widgets/labels/IconLabel.svelte";

	export let icon: IconName;
	export let size: IconSize;
	export let disabled = false;
	export let active = false;
	export let tooltip: string | undefined = undefined;
	export let sharpRightCorners = false;
	// Callbacks
	export let action: (e?: MouseEvent) => void;

	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};

	$: extraClasses = Object.entries(classes)
		.flatMap((classAndState) => (classAndState[1] ? [classAndState[0]] : []))
		.join(" ");
</script>

<button
	class={`icon-button size-${size} ${className} ${extraClasses}`.trim()}
	class:disabled
	class:active
	class:sharp-right-corners={sharpRightCorners}
	on:click={action}
	{disabled}
	title={tooltip}
	tabindex={active ? -1 : 0}
	{...$$restProps}
>
	<IconLabel {icon} />
</button>

<style lang="scss" global>
	.icon-button {
		display: flex;
		justify-content: center;
		align-items: center;
		flex: 0 0 auto;
		margin: 0;
		padding: 0;
		border: none;
		border-radius: 2px;
		background: none;

		svg {
			fill: var(--color-e-nearwhite);
		}

		// The `where` pseudo-class does not contribtue to specificity
		& + :where(.icon-button) {
			margin-left: 0;
		}

		&:hover {
			background: var(--color-6-lowergray);
			color: var(--color-f-white);

			svg {
				fill: var(--color-f-white);
			}
		}

		&.disabled {
			background: none;

			svg {
				fill: var(--color-8-uppergray);
			}
		}

		&.active {
			background: var(--color-e-nearwhite);

			svg {
				fill: var(--color-2-mildblack);
			}
		}

		&.size-12 {
			width: 12px;
			height: 12px;
		}

		&.size-16 {
			width: 16px;
			height: 16px;
		}

		&.size-24 {
			width: 24px;
			height: 24px;
		}

		&.size-32 {
			width: 32px;
			height: 32px;
		}
	}
</style>

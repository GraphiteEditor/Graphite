<template>
	<div class="radio-input" ref="radioInput">
		<button :class="{ active: index === selectedIndex }" v-for="(entry, index) in entries" :key="index" @click="handleEntryClick(entry)" :title="entry.tooltip">
			<IconLabel v-if="entry.icon" :icon="entry.icon" />
			<TextLabel v-if="entry.label">{{ entry.label }}</TextLabel>
		</button>
	</div>
</template>

<style lang="scss">
.radio-input {
	button {
		background: var(--color-5-dullgray);
		fill: var(--color-e-nearwhite);
		height: 24px;
		padding: 0 4px;
		outline: none;
		border: none;
		display: inline-flex;
		align-items: center;

		&:hover {
			background: var(--color-6-lowergray);
			color: var(--color-f-white);

			svg {
				fill: var(--color-f-white);
			}
		}

		&.active {
			background: var(--color-accent);
			color: var(--color-f-white);

			svg {
				fill: var(--color-f-white);
			}
		}

		& + button {
			margin-left: 1px;
		}

		&:first-of-type {
			border-radius: 2px 0 0 2px;
		}

		&:last-of-type {
			border-radius: 0 2px 2px 0;
		}
	}

	.icon-label,
	.text-label {
		display: inline-block;
		vertical-align: top;
	}

	.text-label {
		margin: 0 4px;
	}
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import IconLabel from "@/components/widgets/labels/IconLabel.vue";
import TextLabel from "@/components/widgets/labels/TextLabel.vue";

export interface RadioEntryData {
	value?: string;
	label?: string;
	icon?: string;
	tooltip?: string;
	action?: Function;
}

export type RadioEntries = Array<RadioEntryData>;

export default defineComponent({
	props: {
		entries: { type: Array as PropType<RadioEntries>, required: true },
		selectedIndex: { type: Number, required: true },
	},
	methods: {
		handleEntryClick(menuEntry: RadioEntryData) {
			const index = this.entries.indexOf(menuEntry);
			this.$emit("update:selectedIndex", index);

			if (menuEntry.action) menuEntry.action();
		},
	},
	components: {
		IconLabel,
		TextLabel,
	},
});
</script>

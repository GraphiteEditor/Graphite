<template>
	<button class="icon-button" :class="`size-${String(size)}`" @click="action">
		<IconLabel :icon="icon" />
	</button>
</template>

<style lang="scss">
.icon-button {
	display: inline-flex;
	justify-content: center;
	align-items: center;
	flex: 0 0 auto;
	padding: 0;
	outline: none;
	border: none;
	border-radius: 2px;
	background: none;
	vertical-align: top;
	fill: var(--color-e-nearwhite);

	// The `where` pseduo-class does not contribtue to specificity
	& + :where(.icon-button) {
		margin-left: 0;
	}

	&:hover {
		background: var(--color-6-lowergray);
		color: var(--color-f-white);
		fill: var(--color-f-white);
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

<script lang="ts">
import { defineComponent } from "vue";

import IconLabel from "@/components/widgets/labels/IconLabel.vue";

export default defineComponent({
	props: {
		action: { type: Function, required: true },
		icon: { type: String, required: true },
		size: { type: Number, required: true },
		gapAfter: { type: Boolean, default: false },
	},
	components: { IconLabel },
});
</script>

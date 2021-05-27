<template>
	<button class="icon-button" :class="`size-${String(size)} icon-size-${String(iconSize)}`">
		<slot></slot>
	</button>
</template>

<style lang="scss">
.icon-button {
	display: inline-block;
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

	&.icon-size-12 {
		svg {
			width: 12px;
			height: 12px;
		}

		&.size-16 {
			padding: (16px - 12px) / 2;
		}

		&.size-24 {
			padding: (24px - 12px) / 2;
		}

		&.size-32 {
			padding: (32px - 12px) / 2;
		}
	}

	&.icon-size-16 {
		svg {
			width: 16px;
			height: 16px;
		}

		&.size-24 {
			padding: (24px - 16px) / 2;
		}

		&.size-32 {
			padding: (32px - 16px) / 2;
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

export default defineComponent({
	props: {
		size: { type: Number, required: true },
		iconSize: { type: Number, required: true },
		gapAfter: { type: Boolean, default: false },
	},
});
</script>

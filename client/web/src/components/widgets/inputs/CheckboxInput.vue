<template>
	<div class="checkbox-input">
		<input type="checkbox" :id="`checkbox-${id}`" :checked="checked" @input="(e) => $emit('update:checked', e.target.checked)" />
		<label :for="`checkbox-${id}`">
			<Icon :icon="'Checkmark'" />
		</label>
	</div>
</template>

<style lang="scss">
.checkbox-input {
	input {
		display: none;
	}

	label {
		display: block;
		background: var(--color-e-nearwhite);
		padding: 2px;
		border-radius: 2px;

		&:hover {
			background: var(--color-f-white);
		}

		svg {
			fill: var(--color-2-mildblack);
		}
	}

	input:checked + label {
		background: var(--color-accent);

		&:hover {
			background: var(--color-accent-hover);
		}

		svg {
			fill: var(--color-f-white);
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import Icon from "../labels/Icon.vue";

export default defineComponent({
	data() {
		return {
			id: `${Math.random()}`.substring(2),
		};
	},
	props: {
		checked: { type: Boolean, default: false },
	},
	components: { Icon },
});
</script>

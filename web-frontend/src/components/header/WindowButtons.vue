<template>
	<div class="button minimize" title="Minimize">
		<svg width="10" height="10" viewBox="0 0 10 10">
			<rect y="4" width="10" height="1" />
		</svg>
	</div>
	<div class="button maximize" title="Maximize" v-if="maximized === false">
		<svg width="10" height="10" viewBox="0 0 10 10">
			<path d="M10,0v10H0V0H10z M9,1H1v8h8V1z" />
		</svg>
	</div>
	<div class="button restore-down" title="Restore Down" v-if="maximized === true">
		<svg width="10" height="10" viewBox="0 0 10 10">
			<path d="M10,8H8v2H0V2h2V0h8V8z M7,3H1v6h6V3z M9,1H3v1h5v5h1V1z" />
		</svg>
	</div>
	<div class="button close" title="Close">
		<svg width="10" height="10" viewBox="0 0 10 10">
			<polygon points="10,0.7 9.3,0 5,4.3 0.7,0 0,0.7 4.3,5 0,9.3 0.7,10 5,5.7 9.3,10 10,9.3 5.7,5" />
		</svg>
	</div>
</template>

<style lang="scss">
.button {
	display: flex;
	align-items: center;
	padding: 0 20px;

	svg {
		fill: #ddd;
	}

	&:hover {
		background: #555;

		svg {
			fill: #fff;
		}
	}

	&.close:hover {
		background: #e81123;
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

export default defineComponent({
	name: "WindowButtons",
	props: {
		maximized: { type: Boolean, default: false },
	},
});
</script>

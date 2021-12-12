<template>
	<div style="display: flex">
		<div style="width: 50%">
			<App />
		</div>
		<div style="width: 50%">
			<App />
		</div>
	</div>
</template>

<script lang="ts">
import { defineComponent } from "vue";
import App from "./App.vue";

export default defineComponent({
	components: { App },
});
</script>

<template>
  <div>
    <h3>{{ title }}</h3>
    <figure ref="drawing"></figure>
  </div>
</template>

<script>

import { DrawingExample } from './drawing-example.js'

export default {
  name: 'ExampleComponent',
  props: {
    title: String,
  },
  mounted() {
    new DrawingExample(this.$refs.drawing);
  }
}
</script>

<style scoped>
</style>

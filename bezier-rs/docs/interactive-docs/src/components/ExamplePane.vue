<template>
  <div class="example_row">
    <div v-for="example in examples" :key="example.id">
      <Example :title="example.title" :points="example.points" />
    </div>
  </div>
</template>

<script>

import Example from './Example.vue'

export default {
  name: 'ExamplePane',
  components: {
    Example
  },
  data() {
    return {
      examples: [
        {
          id: 0,
          title: "Quadratic Bezier",
          points: [{x:30, y:30, r:5}, {x:160, y: 170, r:5}, {x:140, y: 20, r:3}],
        },
        {
          id: 1,
          title: "Cubic Bezier",
          points: [{x:30, y:30, r:5}, {x:160, y: 160, r:5}, {x:60, y: 140, r:3}, {x:150, y: 30, r:3}],
        }
      ]
    }
  }
}
</script>

<style>

.example_row {
  display: flex; /* or inline-flex */
  flex-direction: row;
  justify-content: center;
}

</style>

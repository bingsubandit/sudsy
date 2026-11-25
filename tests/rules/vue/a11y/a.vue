<template>
  <div>
    <div @click="a">x</div>
//  ^^^^^^^^^^^^^^^^ click-events-have-key-events
    <div v-on:click.stop="a" @keyup.enter="b">x</div>
    <img :alt="t" src="a.png" />
    <img :src="u" />
//  ^^^^^^^^^^^^^^^^ img-alt
  </div>
</template>

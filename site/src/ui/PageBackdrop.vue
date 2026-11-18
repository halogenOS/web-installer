<!-- SPDX-FileCopyrightText: The halogenOS Project -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->
<template>
  <div aria-hidden="true" class="pointer-events-none fixed inset-0 -z-10 overflow-hidden">
    <div class="absolute inset-x-0 top-0 h-lvh">
      <div class="absolute inset-0 backdrop-night" />
      <div class="absolute inset-0 bg-wedge-outline [clip-path:polygon(var(--wedge-line-shape))]" />
      <div class="absolute inset-0 backdrop-wedge [clip-path:polygon(var(--wedge-shape))]" />
      <div class="absolute inset-0 backdrop-far motion-safe:animate-drift-far" />
      <div class="absolute inset-0 backdrop-near motion-safe:animate-drift-near" />
      <div class="absolute inset-0 backdrop-spore motion-safe:animate-drift-spore" />
    </div>
  </div>
</template>

<!-- SPDX-FileCopyrightText: The halogenOS Project -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->
<script setup lang="ts">
import { onBeforeUnmount } from "vue";
import AppIcon from "./AppIcon.vue";
import { BUTTON_CLASS, ButtonLook } from "./button_look.ts";
import { Icon } from "./icons.ts";

const props = defineProps<{ file: File }>();

const address = URL.createObjectURL(props.file);
onBeforeUnmount(() => {
  URL.revokeObjectURL(address);
});
</script>

<template>
  <a :class="BUTTON_CLASS[ButtonLook.Primary]" :href="address" :download="file.name">
    <AppIcon :name="Icon.Download" />
    <slot />
  </a>
</template>

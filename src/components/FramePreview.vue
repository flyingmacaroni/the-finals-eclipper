<script setup lang="ts">
import usePreviewFrame from "../composables/previewFrame.ts";
import useFileServerAddress from "../composables/fileServerAddress.ts";

const previewFrame = usePreviewFrame();
const server = useFileServerAddress();
</script>

<template>
  <div class="bg-black flex min-h-0 items-center justify-center relative" >
    <div class="absolute inset-0 flex items-center justify-center">
      Processing...
    </div>
    <img v-if="previewFrame" :src="server + `frame/${previewFrame}`" class="absolute w-full h-full object-contain" alt="frame preview" />
  </div>
</template>

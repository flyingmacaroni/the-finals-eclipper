<script setup lang="ts">
import Dialog from "primevue/dialog";
import InputNumber from "primevue/inputnumber";
import InputGroup from "primevue/inputgroup";
import InputGroupAddon from "primevue/inputgroupaddon";
import Textarea from "primevue/textarea";
import Button from "primevue/button";
import Dropdown from "primevue/dropdown";
import {ref} from "vue";
import {open} from "@tauri-apps/plugin-dialog";
import {invoke} from "@tauri-apps/api/core";

const props = defineProps<{
  open: boolean;
}>();

type ImageFilterType = {
  type: "MinMaxRgb",
  min_rgb: [number, number, number],
  max_rgb: [number, number, number],
} | {
  type: "BrightnessContrast",
  brightness: number,
  contrast: number,
}

const imageFilterType = ref<ImageFilterType>(defaultMinMaxRgbFilter());

const scaleHeight = ref(720);
const imagePath = ref<null | string>(null);
const processResult = ref<null | { image: string, ocr_result: string }>(null);
const resizeAlg = ref("Lanczos3");

const RESIZE_ALGORITHMS = [
  "Nearest", "Box", "Bilinear", "Hamming", "CatmullRom", "Mitchell", "Lanczos3",
];

async function selectPhoto() {
// Open a selection dialog for image files
  const selected = await open({
    multiple: false,
    filters: [{
      name: 'Photo',
      extensions: ['jpeg', 'jpg']
    }]
  });

  imagePath.value = selected as string | null;

  process();
}

function process() {
  if (!imagePath.value) {
    return;
  }
  invoke<{ image: string; ocr_result: string }>('process_image', {
    image: imagePath.value,
    imageFilterType: imageFilterType.value,
    scaleHeight: scaleHeight.value,
    resizeAlgorithm: resizeAlg.value
  }).then((result) => processResult.value = result).catch((e) => console.error(e));
}

function defaultMinMaxRgbFilter(): ImageFilterType {
  return {
    type: "MinMaxRgb",
    min_rgb: [215, 215, 215],
    max_rgb: [255, 254, 253],
  }
}

function defaultBrightnessContrastFilter(): ImageFilterType {
  return {
    type: "BrightnessContrast",
    brightness: -0.5, // decrease brightness by 50%
    contrast: 1.5, // increase contrast by 50%
  }
}

function setImageFilterType(type: "MinMaxRgb" | "BrightnessContrast") {
  if (type === imageFilterType.value.type) return;

  if (type === "MinMaxRgb") {
    imageFilterType.value = defaultMinMaxRgbFilter();
    return;
  }
  if (type === "BrightnessContrast") {
    imageFilterType.value = defaultBrightnessContrastFilter();
  }
}

</script>

<template>
  <Dialog @close="$emit('update:open', false)" :visible="props.open" @update:visible="$emit('update:open', false)"
          class="!m-3" content-class="flex flex-col gap-4">
    <div>
      <Button label="Select Photo" @click="selectPhoto"/>
    </div>
    <div class="flex justify-center">
      <img
          v-if="processResult"
          :src="processResult.image"
          class="max-h-[360px] mx-auto"
          alt="image"
      />
    </div>
    <Textarea :value="processResult?.ocr_result" class="min-h-24 font-mono"/>
    <div class="flex flex-col gap-3">
      <label>
        Scale Height
      </label>
      <InputNumber v-model="scaleHeight" :min="160" :max="1080"/>

      <label>
        Scale Method
      </label>
      <Dropdown v-model="resizeAlg" :options="RESIZE_ALGORITHMS"/>
      <label>
        Image Filter Type
      </label>
      <Dropdown
          :model-value="imageFilterType.type"
          :options="['MinMaxRgb', 'BrightnessContrast']"
          @update:model-value="setImageFilterType($event)"
      />
      <template v-if="imageFilterType.type === 'MinMaxRgb'">
        <label>
          Min RGB
        </label>
        <div class="flex flex-row gap-2">
          <InputGroup>
            <InputGroupAddon>
              R
            </InputGroupAddon>
            <InputNumber :min="0" :max="255" v-model="imageFilterType.min_rgb[0]"/>
          </InputGroup>

          <InputGroup>
            <InputGroupAddon>
              G
            </InputGroupAddon>
            <InputNumber :min="0" :max="255" v-model="imageFilterType.min_rgb[1]"/>
          </InputGroup>
          <InputGroup>
            <InputGroupAddon>
              B
            </InputGroupAddon>
            <InputNumber :min="0" :max="255" v-model="imageFilterType.min_rgb[2]"/>
          </InputGroup>
        </div>

        <label class="mt-1">
          Max RGB
        </label>
        <div class="flex flex-row gap-2">
          <InputGroup>
            <InputGroupAddon>
              R
            </InputGroupAddon>
            <InputNumber :min="0" :max="255" v-model="imageFilterType.max_rgb[0]"/>
          </InputGroup>

          <InputGroup>
            <InputGroupAddon>
              G
            </InputGroupAddon>
            <InputNumber :min="0" :max="255" v-model="imageFilterType.max_rgb[1]"/>
          </InputGroup>
          <InputGroup>
            <InputGroupAddon>
              B
            </InputGroupAddon>
            <InputNumber :min="0" :max="255" v-model="imageFilterType.max_rgb[2]"/>
          </InputGroup>
        </div>
      </template>
      <template v-else-if="imageFilterType.type === 'BrightnessContrast'">
        <div class="flex flex-col gap-2">
          <label>Brightness</label>
          <InputNumber :min="-1." :max="1." v-model="imageFilterType.brightness" :max-fraction-digits="5"/>
        </div>
        <div class="flex flex-col gap-2">
          <label>Contrast</label>
          <InputNumber :min="0." :max="20." :step="0.001" v-model="imageFilterType.contrast" :max-fraction-digits="5"/>
        </div>
      </template>
      <template v-else-if="imageFilterType satisfies never"></template>
      <Button @click="process" label="Process"/>
    </div>
  </Dialog>
</template>

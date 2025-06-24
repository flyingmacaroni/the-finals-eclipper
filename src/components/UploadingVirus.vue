<template>
  <div
      class="absolute top-4 right-6 w-[8rem] bg-sky-400 text-black p-1 flex flex-col rounded-md border-2 border-solid border-opacity-100 border-blue-800 box-border gap-1">
    <div class="text-center font-[500] py-2 leading-6 text-sm">
      CNS Uploading Virus to VAIIYA
    </div>
    <div class="font-bold text-center">
      Please wait...
    </div>
    <div class="border border-solid h-4 rounded-sm bg-white overflow-hidden p-0.5">
      <div class="bg-[#12f] h-full w-9 progress-bar">

      </div>
    </div>
  </div>
</template>

<style scoped>
@keyframes progress-bar {
  from {
    transform: translate3d(-2.25rem, 0, 0);
  }
  to {
    transform: translate3d(calc(100% + 3 * 2.25rem), 0, 0);
  }
}

.progress-bar {
  animation: 2s steps(20) 0s infinite normal none running progress-bar;
}
</style>
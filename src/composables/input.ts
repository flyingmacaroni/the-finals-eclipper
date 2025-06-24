import {ref, watch} from "vue";

const input = ref<null | string>(null);

export default function useInput() {
    if (input.value === null) input.value = window.localStorage.getItem('input');

    watch(input, (input) => {
        if (input !== null) window.localStorage.setItem('input', input);
        else window.localStorage.removeItem('input');
    });

    return input;
}

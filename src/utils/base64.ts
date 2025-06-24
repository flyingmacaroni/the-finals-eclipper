export function base64ToBytes(base64: string) {
    const binString = atob(base64);
    return stringToUnit8Array(binString);
}

export function stringToUnit8Array(value: string) {
    // @ts-ignore
    return Uint8Array.from(value, (m) => m.codePointAt(0));
}

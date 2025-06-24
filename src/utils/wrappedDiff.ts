export default function wrappedDiff(a: number, b: number, max: number) {
    if (a - b < 0) {
        return max - b + a;
    }
    return a - b;
}

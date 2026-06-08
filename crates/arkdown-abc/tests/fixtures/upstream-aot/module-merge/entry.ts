// Fixture: module source (ESM, merge-abc)
// Upstream oracle: es2abc --module --merge-abc --output modules.abc @filesInfo.txt
export const value = 42;

export function helper(): number {
    return value + 1;
}

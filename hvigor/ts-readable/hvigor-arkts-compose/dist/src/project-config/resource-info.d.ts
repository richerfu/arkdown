/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/project-config/resource-info.d.ts
 */
export interface ResourceInfo {
    /**
     * resourceTable.txt的路径
     */
    appResource?: string;
    /**
     * 编译资源的产物中rawfile的路径
     */
    rawFileResource?: string;
    /**
     * appResource字段对应的文件的hash值
     */
    resourceTableHash?: string;
}

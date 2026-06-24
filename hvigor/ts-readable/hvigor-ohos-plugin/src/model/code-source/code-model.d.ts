/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/code-source/code-model.d.ts
 */
import { CodeType } from '../../enum/code-type-enum';
export interface CodeModel {
    /**
     * 获取该源码的路径
     */
    getSrcPath: () => string;
    /**
     * 获取该源码的类型
     */
    getCodeType: () => CodeType;
}

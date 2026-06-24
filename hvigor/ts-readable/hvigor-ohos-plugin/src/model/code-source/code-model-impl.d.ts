/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/code-source/code-model-impl.d.ts
 */
import { CodeType } from '../../enum/code-type-enum';
import { CodeModel } from './code-model';
/**
 * 源码集合模型
 *
 * @since 2022/2/23
 */
export declare class CodeModelImpl implements CodeModel {
    private readonly _codeType;
    private readonly _srcPath;
    constructor(srcPath: string, codeType: CodeType);
    getSrcPath(): string;
    getCodeType(): CodeType;
}

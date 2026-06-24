/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/source-set/core-source-set-impl.d.ts
 */
import { CodeType } from '../../enum/code-type-enum';
import { CodeModel } from '../code-source/code-model';
import { CoreResModelImpl } from '../res/core-res-model-impl';
import { SourceSetModel } from './source-set-model';
/**
 * module中每个target对应的一个sourceSet对象
 *
 * @since 2022/2/24
 */
export declare abstract class CoreSourceSetImpl implements SourceSetModel {
    protected readonly sourceSetRoot: string;
    protected readonly codeMap: Map<CodeType, CodeModel>;
    protected readonly targetResModel: CoreResModelImpl;
    constructor(sourceSetRoot: string, targetResModel: CoreResModelImpl);
    getSourceSetRoot(): string;
    getCodeMap(): Map<CodeType, CodeModel>;
    getTargetResPath(): string;
    private initCodeMap;
    abstract getTargetSourceSetModuleName(): string | undefined;
}

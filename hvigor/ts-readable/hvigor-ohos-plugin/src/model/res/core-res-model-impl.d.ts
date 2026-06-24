/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/res/core-res-model-impl.d.ts
 */
import { ResModel } from './res-model';
/**
 * 不同target对应的资源集合模型
 *
 * @since 2022/2/23
 */
export declare class CoreResModelImpl implements ResModel {
    private readonly jsonPath;
    private readonly resourcePath;
    constructor(jsonPath: string, resourcePath: string);
    getJsonContent(): string;
    getResourcePath(): string;
    getJsonPath(): string;
}

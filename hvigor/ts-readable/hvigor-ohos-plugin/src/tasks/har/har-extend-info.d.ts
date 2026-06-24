/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/har/har-extend-info.d.ts
 */
import { ModuleModel } from '../../model/module/module-model';
import { ExternalNativeOpt } from '../../options/build/build-opt';
import { BuildOpt } from '../../options/build/build-opt';
export declare enum ArtifactType {
    ORIGINAL = "original",
    OBFUSCATION = "obfuscation",
    BYTECODE = "bytecode"
}
export declare class HarExtendInfo {
    private readonly moduleModel;
    readonly artifactType: string | undefined;
    readonly nativeOption?: ExternalNativeOpt;
    readonly buildOpt: BuildOpt;
    constructor(moduleModel: ModuleModel, buildOption: BuildOpt);
    hasConfiguredNativeOption(): boolean;
    isObfuscatedHar(): boolean;
    getArtifactType(): string;
}

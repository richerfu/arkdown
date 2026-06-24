/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-pre-build.d.ts
 */
import { FileSet } from '@ohos/hvigor';
import { JsonProfile } from '../../model/json-profile';
import { AbstractPreBuild } from '../abstract/abstract-pre-build';
import { TargetTaskService } from '../service/target-task-service';
/**
 * Fa module preBuild Task
 *
 * @since 2022/9/8
 */
export declare class LegacyPreBuild extends AbstractPreBuild {
    private readonly targetJsonPath;
    private readonly targetConfigOptObj;
    private logger;
    private readonly configOptObj;
    private get isExpandImportEnabled();
    declareInputFiles(): FileSet;
    constructor(taskService: TargetTaskService);
    protected doValidateForDiffApiType(): void;
    protected getJsonProfileByModel(): JsonProfile;
    private validateForm;
    private validateJsType;
    private validateAbilities;
    /**
     * AtomicService相关校验
     *
     * @private
     */
    validateAtomicService(): void;
    initTaskDepends(): void;
    /**
     * 检查差异化的sourceRoot，预留
     */
    protected checkExtendSourceDirs(): void;
    /**
     * 检查byteCodeHar，预留
     */
    protected checkByteCodeHar(): void;
    protected validatePreloadSystemSo(): void;
    protected checkExpandImportConfigs(): void;
    /**
     * 分布式卡片的校验，fa模型不涉及
     */
    protected checkFormModule(): void;
}

/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/sign-hap.d.ts
 */
import { FileSet } from '@ohos/hvigor';
import { BaseSign } from './base/base-sign';
import { TargetTaskService } from './service/target-task-service';
/**
 * Sign hap
 *
 * @since 2022/1/10
 */
export declare class SignHap extends BaseSign {
    /**
     * 签名入参对象集合
     *
     * @private
     */
    private readonly signModelList;
    constructor(targetService: TargetTaskService);
    taskShouldDo(): boolean;
    declareInputFiles(): FileSet;
    declareOutputFiles(): FileSet;
    protected doTaskAction(): Promise<void>;
    initTaskDepends(): void;
    /**
     * 初始化签名入参对象集合
     *
     * @private
     */
    private initSignModelData;
    /**
     * 根据传入entryName，添加signModel签名入参对象
     *
     * @param entryName
     * @private
     */
    private addSignModel;
}

/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/sign-module-remote-hsp.d.ts
 */
import { FileSet } from '@ohos/hvigor';
import { TargetTaskService } from './service/target-task-service';
import { SignRemoteHsp } from './sign-remote-hsp';
import { OhosLogger } from '../utils/log/ohos-logger';
export declare class SignModuleRemoteHsp extends SignRemoteHsp {
    protected readonly _log: OhosLogger;
    constructor(targetService: TargetTaskService);
    taskShouldDo(): boolean;
    /**
     * 判断全局变量promise的值执行不同操作
     * projectUnsigned说明工程级remote_hsp文件不存在或SignProjectRemoteHsp增量未执行，直接执行模块级remote_hsp签名
     * typeof promise为function说明工程级remote_hsp正在签名，所以模块级等待签名结束
     * projectSignSuccesses说明工程级remote_hsp签名成功，直接执行模块级remote_hsp签名
     * @protected
     */
    protected doTaskAction(): Promise<void>;
    private moduleSignAction;
    declareInputFiles(): FileSet;
    declareOutputFiles(): FileSet;
    initTaskDepends(): void;
}

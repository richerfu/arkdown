/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/packing-check/packing-check-har.d.ts
 */
import { OhosLogger } from '../../utils/log/ohos-logger';
import { TargetTaskService } from '../service/target-task-service';
import { OhosHarTask } from '../task/ohos-har-task';
/**
 * ObfuscatedHar或者ByteCodeHar在打包前，对包产物进行检查
 * originalHar里面全是源码，不需要检查
 *
 */
export declare class PackingCheckHar extends OhosHarTask {
    readonly _log: OhosLogger;
    moduleRoot: string;
    private readonly releaseWhiteListTemDir;
    private readonly byteCodeHarAbcOutputDir;
    private readonly originalEtsPath;
    private readonly packageHarTempDir;
    constructor(taskService: TargetTaskService);
    taskShouldDo(): boolean;
    protected doTaskAction(): void;
    initTaskDepends(): void;
    /**
     * 收集用于字节码HAR的ets下源代码文件。
     */
    private collectSourceCodeFilesForByteCodeHar;
    /**
     * 收集用于混淆HAR的源代码文件。
     * 检查范围：打包目录taskDir下全部文件
     * 检查规则改变：去除检查ts\js文件，并且不是.d.ets文件才命中
     */
    private collectSourceCodeFilesForObfuscatedHar;
}

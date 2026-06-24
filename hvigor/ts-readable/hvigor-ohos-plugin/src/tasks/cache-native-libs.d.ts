/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/cache-native-libs.d.ts
 */
import { TaskInputValue } from '@ohos/hvigor';
import { AbstractNativeStrip } from './abstract/abstract-native-strip';
import { TargetTaskService } from './service/target-task-service';
/**
 * build-native-strip任务执行完执行这个任务把本次的binxo和strip结果落盘保存
 */
export declare class CacheNativeLibs extends AbstractNativeStrip {
    private logger;
    declareInputs(): Map<string, TaskInputValue>;
    private get cacheStrippedNativeLibs();
    private get cacheBinxoNativeLibs();
    private get buildCacheFilePath();
    private get binxoCacheFilePath();
    constructor(targetService: TargetTaskService);
    protected doTaskAction(): Promise<void>;
    initTaskDepends(): void;
}

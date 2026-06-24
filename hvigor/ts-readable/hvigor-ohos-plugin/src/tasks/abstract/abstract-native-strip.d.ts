/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/abstract/abstract-native-strip.d.ts
 */
import { TaskDetails } from '@ohos/hvigor';
import { DebugSymbol, NativeLib } from '../../options/build/build-opt';
import { BinxoValidationService } from '../../utils/validate/binxo/binxo-validation-service';
import { AbstractBuildNative } from '../abstract-build-native';
import { TargetTaskService } from '../service/target-task-service';
/**
 * CacheNativeLibs和DoNativeStrip的抽象父类
 *
 * @since 2023/11/30
 */
export declare abstract class AbstractNativeStrip extends AbstractBuildNative {
    protected readonly nativeLibOption: NativeLib | undefined;
    protected readonly excludeSoFromBinxo: string[] | undefined;
    protected readonly debugSymbol: DebugSymbol | undefined;
    protected readonly exclude: string[] | undefined;
    protected readonly intermediatesProcessLibs: string;
    protected readonly doNativeBinxoLibs: string;
    protected readonly strippedNativeLibs: string;
    protected readonly moduleBuildPath: string;
    protected readonly cacheFilePath: string;
    protected readonly cacheBinxoFilePath: string;
    protected readonly binxoValidationService: BinxoValidationService;
    protected constructor(targetService: TargetTaskService, taskDetails: TaskDetails);
    /**
     * 判断当前 binxo 配置是否有效
     * 依赖于 hwAsan 与 binxo 同时启用
     *
     * @returns true if binxo is valid and enabled; otherwise false.
     */
    protected isBinxoValid(): boolean;
}

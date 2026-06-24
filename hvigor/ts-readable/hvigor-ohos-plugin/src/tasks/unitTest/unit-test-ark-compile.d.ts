/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/unitTest/unit-test-ark-compile.d.ts
 */
import { FileSet } from '@ohos/hvigor';
import { ProjectConfig } from '@ohos/hvigor-arkts-compose';
import { OhosLogger } from '../../utils/log/ohos-logger';
import { ArkCompile } from '../ark-compile';
/**
 * 单元测试ArkTS编译 stage模型
 *
 * @since 2023/4/23
 */
export declare class UnitTestArkCompile extends ArkCompile {
    logger: OhosLogger;
    declareInputFiles(): FileSet;
    protected allowToPreloadSystemSo(): boolean;
    protected doTaskAction(): Promise<void>;
    protected initDefaultArkCompileConfig(): Promise<ProjectConfig>;
    initTaskDepends(): void;
    /**
     *落盘buildConfig文件，传递abc生成路径给测试矿机
     */
    flushBuildConfig(): void;
}

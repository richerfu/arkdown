/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/unit-test/legacy-unit-test-ark-compile.d.ts
 */
import { FileSet } from '@ohos/hvigor';
import { ProjectConfig } from '@ohos/hvigor-arkts-compose';
import { OhosLogger } from '../../../utils/log/ohos-logger';
import { LegacyArkCompile } from '../legacy-ark-compile';
/**
 * 单元测试ArkTS编译 fa模型
 *
 * @since 2023/4/23
 */
export declare class LegacyUnitTestArkCompile extends LegacyArkCompile {
    logger: OhosLogger;
    declareInputFiles(): FileSet;
    protected initDefaultArkCompileConfig(): Promise<ProjectConfig>;
    initTaskDepends(): void;
    protected doTaskAction(): Promise<void>;
}

/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/unitTest/unit-test-copy-mock-config-json.d.ts
 */
import { TargetTaskService } from '../service/target-task-service';
import { TestCopyMockConfigJson } from '../task/test-copy-mock-config-json';
/**
 * 测试框架执行mock时要把 build/default/intermediates/res/{default}/mock-config.json 。
 * mock-config.json 是由ets-loader生成到 intermediates 目录下的。
 * 拷贝到测试包 hap 中的resources/rawfile/mock/mock-config.json 。
 */
export declare class UnitTestCopyMockConfigJson extends TestCopyMockConfigJson {
    constructor(taskService: TargetTaskService);
    initTaskDepends(): void;
}

/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/task/test-copy-mock-config-json.d.ts
 */
import { OhosHapTask } from './ohos-hap-task';
/**
 * 测试框架执行mock时要把 build/default/intermediates/res/{default}/mock-config.json 。
 * mock-config.json 是由ets-loader生成到 intermediates 目录下的。
 * 拷贝到测试包 hap 中的resources/rawfile/mock/mock-config.json 。
 */
export declare abstract class TestCopyMockConfigJson extends OhosHapTask {
    private _log;
    protected get mockConfigJsonFile(): string;
    protected get mockConfigJsonOutputFile(): string;
    protected doTaskAction(): void;
    protected copyMockConfigJson(): void;
}

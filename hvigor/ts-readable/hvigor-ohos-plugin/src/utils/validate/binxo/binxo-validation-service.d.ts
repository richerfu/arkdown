/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/validate/binxo/binxo-validation-service.d.ts
 */
import { TargetTaskService } from '../../../tasks/service/target-task-service';
/**
 * 提供 binxo 与 hwAsan 配置状态的判断与验证服务
 *
 * @since 2026/03/05
 */
export declare class BinxoValidationService {
    private readonly hwAsanConfig;
    private readonly binxoConfig;
    private readonly hwAsanCmake;
    private readonly binxoCmake;
    /**
     * 构造函数，初始化 binxo 和 hwAsan 的配置状态
     *
     * @param targetService
     */
    constructor(targetService: TargetTaskService);
    /**
     * 判断 binxo是否有效启用，条件为：hwAsan 与 binxo 均启用
     *
     * @returns true if both hwAsan and binxo are enabled; otherwise false.
     */
    isBinxoValid(): boolean;
}

/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/validate/binxo/binxo-validation-service-factory.d.ts
 */
import { TargetTaskService } from '../../../tasks/service/target-task-service';
import { BinxoValidationService } from './binxo-validation-service';
/**
 * BinxoValidationService 的简单工厂类，负责创建 BinxoValidationService 实例
 *
 * @since 2026/03/05
 */
export declare class BinxoValidationServiceFactory {
    /**
     * 创建新的 BinxoValidationService 实例
     *
     * @param targetService
     * @returns BinxoValidationService 实例
     */
    static createBinxoValidationService(targetService: TargetTaskService): BinxoValidationService;
}

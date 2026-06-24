/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/distribution/api-version-handler.d.ts
 */
import { DistroFilterConfig, DistroFilterHandler } from './distro-filter-handler';
/**
 * apiVersion对象校验
 * policy:include/exclude
 * value:[3,30]
 */
export declare class ApiVersionHandler extends DistroFilterHandler {
    validateDistroFilterRequest(sourceDistroFilterConfig: DistroFilterConfig, targetDistroFilterConfig: DistroFilterConfig): void;
    getDimensionValue(distroFilterConfig: DistroFilterConfig): number[] | undefined;
    private getApiVersionScope;
}

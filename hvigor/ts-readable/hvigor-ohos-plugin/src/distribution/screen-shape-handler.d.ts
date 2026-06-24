/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/distribution/screen-shape-handler.d.ts
 */
import { DistroFilterConfig, DistroFilterHandler } from './distro-filter-handler';
/**
 * screenShape对象校验
 * policy:include/exclude
 * value:["rect","circle"]
 */
export declare class ScreenShapeHandler extends DistroFilterHandler {
    validateDistroFilterRequest(sourceDistroFilterConfig: DistroFilterConfig, targetDistroFilterConfig: DistroFilterConfig): void;
    getDimensionValue(distroFilterConfig: DistroFilterConfig): string[] | undefined;
}

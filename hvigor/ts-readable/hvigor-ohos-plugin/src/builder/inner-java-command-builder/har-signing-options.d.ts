/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/builder/inner-java-command-builder/har-signing-options.d.ts
 */
import { SigningOptionsV2 } from './signing-options-v2';
/**
 * 签名工具sdk-sign-tool的签名options
 *
 * @since 2024/05/11
 */
export declare class HarSigningOptions extends SigningOptionsV2 {
    sign(): this;
    addAppCertFile(certPath: string): this;
}

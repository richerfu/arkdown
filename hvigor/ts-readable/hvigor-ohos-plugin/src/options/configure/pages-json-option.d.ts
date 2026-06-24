/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/options/configure/pages-json-option.d.ts
 */
import { Option } from '../options';
/**
 * pages.json object
 */
export declare namespace PagesJsonOption {
    interface PageObj extends Option {
        src: string[];
        window?: WindowObj;
    }
    interface WindowObj extends Option {
        designWidth: number;
        autoDesignWidth: boolean;
    }
}

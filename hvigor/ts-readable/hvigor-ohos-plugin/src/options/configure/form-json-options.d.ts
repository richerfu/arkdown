/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/options/configure/form-json-options.d.ts
 */
import { Option } from '../options';
export declare namespace FormConfigJson {
    interface FormConfigJsonObj {
        forms?: FormConfig[];
    }
    interface FormConfig extends Option {
        uiSyntax?: 'arkts' | 'hml';
        src: string;
    }
}

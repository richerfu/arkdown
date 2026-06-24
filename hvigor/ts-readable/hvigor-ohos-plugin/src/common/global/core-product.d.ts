/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/common/global/core-product.d.ts
 */
import { ProjectModel } from '../../model/project/project-model';
import { ProjectBuildProfile } from '../../options/build/project-build-profile';
import { Product } from '../../plugin/context/plugin-context';
import ProductBuildOpt = ProjectBuildProfile.ProductBuildOpt;
export declare class CoreProduct implements Product {
    private productBuildOpt;
    private productName;
    private bundleName;
    private bundleType;
    private projectModel;
    private outputArtifactName;
    constructor(productBuildOpt: ProductBuildOpt, projectModel: ProjectModel);
    transformToCoreProduct(): void;
    getBundleName(): string;
    private _getBundleName;
    getBundleType(): string;
    private _getBundleType;
    getProductName(): string;
    private _getProductName;
    getOutputArtifactName(): string | undefined;
    private _getOutputArtifactName;
}

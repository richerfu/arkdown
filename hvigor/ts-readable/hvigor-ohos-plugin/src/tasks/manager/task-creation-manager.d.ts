/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/manager/task-creation-manager.d.ts
 */
import { CoreTask, TaskDetails } from '@ohos/hvigor';
import { BeforeGenerateDeviceCoverage } from '../before-generate-device-coverage';
import { BuildProfileTask } from '../build-profile-task';
import { CollectDebugSymbol } from '../collect-debug-symbol';
import { GenerateDeviceCoverage } from '../generate-device-coverage';
import { GenerateLoaderJson } from '../generate-loader-json';
import { GenerateMetadata } from '../generate-metadata';
import { GeneratePkgContextInfo } from '../generate-pkg-context-info';
import { GeneratePkgModuleJson } from '../generate-pkg-module-json';
import { GeneratePkgSdkInfo } from '../generate-pkg-sdk-info';
import { ArkLinter } from '../har/ark-linter';
import { HarBuildProfileTask } from '../har/har-build-profile-task';
import { PackageSignHar } from '../har/package-sign-har';
import { ProcessObfuscationFiles } from '../har/process-obfuscation-files';
import { FastPackageHsp } from '../hsp/fast-package-hsp';
import { PackageHsp } from '../hsp/package-hsp';
import { PackageSharedHar } from '../hsp/package-shared-har';
import { PackageSharedTgz } from '../hsp/package-shared-tgz';
import { PrepareSharedHarRes } from '../hsp/prepare-shared-har-res';
import { SignHsp } from '../hsp/sign-hsp';
import { MakePackInfo } from '../make-pack-info';
import { MergeProfile } from '../merge-profile';
import { CreateHarModuleInfo } from '../module-info/create-har-module-info';
import { CreateModuleInfo } from '../module-info/create-module-info';
import { GenerateOhosTestTemplate } from '../ohosTest/generate-ohos-test-template';
import { OhosTestCopyMockConfigJson } from '../ohosTest/ohos-test-copy-mock-config-json';
import { ProcessModulePrivacyProfile } from '../pac-process/process-module-privacy-profile';
import { PackageHap } from '../package-hap';
import { PackageHqf } from '../package-hqf';
import { PackingCheckHar } from '../packing-check/packing-check-har';
import { PackingCheck } from '../packing-check/packing-check';
import { PreBuild } from '../pre-build';
import { PreCheckSyscap } from '../pre-check-syscap';
import { PreviewCompileResource } from '../preview-compile-resource';
import { PreviewUpdateAssets } from '../preview-update-assets';
import { CopyPreviewProfile } from '../previewer/copy-preview-profile';
import { ReplacePreviewerPage } from '../previewer/replace-previewer-page';
import { ProcessCompiledResources } from '../process-compiled-resources';
import { ProcessIntegratedHsp } from '../process-integrated-hsp';
import { ProcessProfile } from '../process-profile';
import { ProcessRouterMap } from '../process-router-map';
import { ProcessShareConfig } from '../process-share-config';
import { ProcessStartupConfig } from '../process-startup-config';
import { TargetTaskService } from '../service/target-task-service';
import { SignHap } from '../sign-hap';
import { SignHqf } from '../sign-hqf';
import { SignModuleRemoteHsp } from '../sign-module-remote-hsp';
import { SignProjectRemoteHsp } from '../sign-project-remote-hsp';
import { SyscapTransform } from '../syscap-transform';
import { TargetTaskCreator } from '../task-creator';
import { BuildUnitTestHook } from '../unitTest/build-unit-test-hook';
import { GenerateUnitTestResult } from '../unitTest/generate-unit-test-result';
import { GenerateUnitTestTemplate } from '../unitTest/generate-unit-test-template';
import { ReplaceSourcemapInCoverageMode } from '../unitTest/replace-sourcemap-in-coverage-mode';
import { ReplaceUnitTestIndexFile } from '../unitTest/replace-unit-test-index-file';
import { UnitTestCopyMockConfigJson } from '../unitTest/unit-test-copy-mock-config-json';
import { UnitTestProcessProfile } from '../unitTest/unit-test-process-profile';
export declare class OhosTestCopyMockConfigJsonCA extends TargetTaskCreator {
    provider: () => OhosTestCopyMockConfigJson;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class UnitTestCopyMockConfigJsonCA extends TargetTaskCreator {
    provider: () => UnitTestCopyMockConfigJson;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class PreBuildCA extends TargetTaskCreator {
    provider: () => PreBuild;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
    afterConfigure: () => void;
}
export declare class GenerateUnitTestTemplateCA extends TargetTaskCreator {
    provider: () => GenerateUnitTestTemplate;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class GenerateOhosTestTemplateCA extends TargetTaskCreator {
    provider: () => GenerateOhosTestTemplate;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class GenerateUnitTestResultCA extends TargetTaskCreator {
    provider: () => GenerateUnitTestResult;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class GenerateDeviceCoverageCA extends TargetTaskCreator {
    provider: () => GenerateDeviceCoverage;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class BeforeGenerateDeviceCoverageCA extends TargetTaskCreator {
    provider: () => BeforeGenerateDeviceCoverage;
    declareDepends(): string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class GeneratePkgContextInfoCA extends TargetTaskCreator {
    provider: () => GeneratePkgContextInfo;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class GeneratePkgSdkInfoCA extends TargetTaskCreator {
    provider: () => GeneratePkgSdkInfo;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class ProcessProfileCA extends TargetTaskCreator {
    provider: () => ProcessProfile;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class MergeProfileCA extends TargetTaskCreator {
    provider: () => MergeProfile;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
    afterConfigure: () => string[];
}
export declare class HarBuildProfileTaskCA extends TargetTaskCreator {
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
    provider: () => HarBuildProfileTask;
}
export declare class CreateHarModuleInfoCA extends TargetTaskCreator {
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
    provider: () => CreateHarModuleInfo;
}
export declare class CreateModuleInfoCA extends TargetTaskCreator {
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
    provider: () => CreateModuleInfo;
}
export declare class GenerateLoaderJsonCA extends TargetTaskCreator {
    provider: () => GenerateLoaderJson;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class ProcessRouterMapCA extends TargetTaskCreator {
    provider: () => ProcessRouterMap;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class ProcessShareConfigCA extends TargetTaskCreator {
    provider: () => ProcessShareConfig;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class ProcessStartupConfigCA extends TargetTaskCreator {
    provider: () => ProcessStartupConfig;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class PackageHapCA extends TargetTaskCreator {
    provider: () => PackageHap;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class ProcessCompiledResourcesCA extends TargetTaskCreator {
    provider: () => ProcessCompiledResources;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class ProcessIdDefinedFileCA extends TargetTaskCreator {
    provider: () => CoreTask;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class FastPackageCA extends TargetTaskCreator {
    provider: () => CoreTask;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class PackageHqfCA extends TargetTaskCreator {
    provider: () => PackageHqf;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class SignHqfCA extends TargetTaskCreator {
    provider: () => SignHqf;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class GeneratePkgModuleJsonCA extends TargetTaskCreator {
    provider: () => GeneratePkgModuleJson;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class CollectDebugSymbolCA extends TargetTaskCreator {
    provider: () => CollectDebugSymbol;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class PackageHspCA extends TargetTaskCreator {
    provider: () => PackageHsp;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class FastPackageHspCA extends TargetTaskCreator {
    provider: () => FastPackageHsp;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class SignHspCA extends TargetTaskCreator {
    provider: () => SignHsp;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class PackageSignHarCA extends TargetTaskCreator {
    provider: () => PackageSignHar;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class SignProjectRemoteHspCA extends TargetTaskCreator {
    provider: () => SignProjectRemoteHsp;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class SignModuleRemoteHspCA extends TargetTaskCreator {
    provider: () => SignModuleRemoteHsp;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class ProcessIntegratedHspCA extends TargetTaskCreator {
    provider: () => ProcessIntegratedHsp;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class PackageSharedHarCA extends TargetTaskCreator {
    provider: () => PackageSharedHar;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class PrepareSharedHarResCA extends TargetTaskCreator {
    provider: () => PrepareSharedHarRes;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class PackageSharedTgzCA extends TargetTaskCreator {
    provider: () => PackageSharedTgz;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class PreviewCompileResourceCA extends TargetTaskCreator {
    provider: () => PreviewCompileResource;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class PreviewUpdateAssetsCA extends TargetTaskCreator {
    provider: () => PreviewUpdateAssets;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
    afterConfigure: () => void;
}
export declare class ReplacePreviewerPageCA extends TargetTaskCreator {
    provider: () => ReplacePreviewerPage;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class CopyPreviewProfileCA extends TargetTaskCreator {
    provider: () => CopyPreviewProfile;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class ReplaceUnitTestIndexFileCA extends TargetTaskCreator {
    provider: () => ReplaceUnitTestIndexFile;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class BuildUnitTestHookCA extends TargetTaskCreator {
    provider: () => BuildUnitTestHook;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class ReplaceSourceMapInCoverageModeCA extends TargetTaskCreator {
    provider: () => ReplaceSourcemapInCoverageMode;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class SignHapCA extends TargetTaskCreator {
    provider: () => SignHap;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class PreCheckSyscapCA extends TargetTaskCreator {
    provider: () => PreCheckSyscap;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class SyscapTransformCA extends TargetTaskCreator {
    provider: () => SyscapTransform;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class PackageHarCA extends TargetTaskCreator {
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
    provider: () => CoreTask;
}
export declare class ProcessHarArtifactsCA extends TargetTaskCreator {
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
    provider: () => CoreTask;
}
export declare class ProcessOhPackageJsonCA extends TargetTaskCreator {
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
    provider: () => CoreTask;
}
export declare class ProcessPackageJsonCA extends TargetTaskCreator {
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
    provider: () => CoreTask;
}
export declare class ArkLinterCA extends TargetTaskCreator {
    declareDepends: () => string[];
    declareTaskDetail: () => {
        name: string;
    };
    provider: () => ArkLinter;
}
export declare class PackingCheckCA extends TargetTaskCreator {
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
    provider: () => PackingCheck;
}
export declare class PackingCheckHarCA extends TargetTaskCreator {
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
    provider: () => PackingCheckHar;
}
export declare class ProcessObfuscationFilesCA extends TargetTaskCreator {
    declareDepends: () => string[];
    declareTaskDetail: () => {
        name: string;
    };
    provider: () => ProcessObfuscationFiles;
}
export declare class GenerateMetadataCA extends TargetTaskCreator {
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
    provider: () => GenerateMetadata;
}
export declare class MakePackInfoCA extends TargetTaskCreator {
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
    provider: () => MakePackInfo;
}
export declare class UnitTestProcessProfileCA extends TargetTaskCreator {
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
    provider: () => UnitTestProcessProfile;
}
export declare class BuildProfileTaskCA extends TargetTaskCreator {
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
    provider: () => BuildProfileTask;
}
/**
 * 获取依赖中的CreateBuildProfile、CREATE_HAR_BUILD_PROFILE任务
 */
export declare function getBuildProfileEtsDependencies(targetService: TargetTaskService): string[];
export declare class ProcessModulePrivacyProfileCA extends TargetTaskCreator {
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
    provider: () => ProcessModulePrivacyProfile;
}

/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/options/build/project-build-profile.d.ts
 */
import { RequiredNamed } from '../options';
import { BuildOpt } from './build-opt';
/**
 * 工程级别的build-profile.json5的pattern
 * 所有需要配到project.json5的数据通过对应接口提供，
 * 若该接口中未定义，则认为是开发者手动加了不支持的字段(开发态无法联想)
 *
 * 命名规范: 每一个子标签都以xxxBuildOpt结尾
 *
 */
export declare namespace ProjectBuildProfile {
    interface MaterialBuildOpt {
        storeFile: string;
        storePassword: string;
        keyAlias: string;
        keyPassword: string;
        signAlg: string;
        profile: string;
        certpath: string;
    }
    interface SigningConfigBuildOpt extends RequiredNamed {
        material: MaterialBuildOpt;
        type?: string;
    }
    interface ProductBuildOpt extends RequiredNamed {
        signingConfig?: string;
        bundleName?: string;
        buildOption?: BuildOpt;
        compileSdkVersion?: number | string;
        compatibleSdkVersion?: number | string;
        targetSdkVersion?: number | string;
        runtimeOS?: string;
        bundleType?: string;
        label?: string;
        versionCode?: number;
        versionName?: string;
        buildVersion?: string;
        icon?: string;
        resource?: ProductResourceObj;
        output?: OutputOpt;
        arkTSVersion?: string;
        vendor?: string;
        compatibleSdkVersionStage?: string;
    }
    interface ProductResourceObj {
        directories: string[];
    }
    interface ProductApiMeta {
        compileSdkVersion: ApiMeta;
        compatibleSdkVersion: ApiMeta;
        targetSdkVersion: ApiMeta | undefined;
        originCompatibleSdkVersion: string | number | undefined;
    }
    /**
     * API数据
     * type:
     *   0: 10(非点分制) or "20.1.1"(点分制)
     *   1: 4.0.0(10)(非点分制) or “6.1.1(20.1.1)" (点分制)
     * api 字符串版本
     *   0: '10' or "20.1.1"
     *   1: '4.0.0' or “6.1.1"
     * type API数据类型 0: number 1: string
     * fullVersion: 用户配置的原始api版本号
     *   hos："10"(非点分制) or "20.1.1"(点分制)
     *   oh："10"(非点分制) or "20.1.1"(点分制)
     *   1. majorVersion、minorVersion、patchVersion均为整型，最大值为999
     *   2. majorVersion不会为0；
     *   3. 仅有majorVersion和minorVersion存在时，则minorVersion不会为0；
     *   4. majorVersion、minorVersion、patchVersion均存在时，则patchVersion不会为0
     *   5. majorVersion、minorVersion、patchVersion均不会存在前导0，即不会出01场景
     *   6. 两个API Level大小比较时，依次比较majorVersion、minorVersion、patchVersion
     * major：点分制版本号第一位
     * minor：点分制版本号第二位
     * patch：点分制版本号第三位
     */
    interface ApiMeta {
        api: string;
        version: number;
        type: ApiValType;
        fullVersion: string;
        major: number;
        minor: number | undefined;
        patch: number | undefined;
    }
    enum ApiValType {
        'NUM' = 0,
        'STRING' = 1
    }
    interface AppBuildOpt {
        supportHos?: boolean;
        signingConfigs?: SigningConfigBuildOpt[];
        compileSdkVersion?: number;
        compatibleSdkVersion?: number;
        products?: ProductBuildOpt[];
        multiProjects?: boolean;
        buildModeSet?: BuildMode[];
    }
    interface ProjectTargetBuildOpt extends RequiredNamed {
        applyToProducts: string[];
    }
    interface ModuleBuildOpt extends RequiredNamed {
        srcPath: string;
        targets?: ProjectTargetBuildOpt[];
        belongProjectPath?: string;
    }
    interface ProjectProfileOpt {
        app: AppBuildOpt;
        modules: ModuleBuildOpt[];
        crossplatform: boolean;
    }
    interface BuildMode extends RequiredNamed {
        buildOption?: BuildOpt;
    }
    interface ModuleRuntimeOS {
        targetRunTimeOS?: string;
        targetMsg?: string;
    }
    interface RemoteHspOpt {
        hspName: string;
        hspPath: string;
        hspVersion: string;
        hspFileName: string;
        hspDirName: string;
        isIntegratedHsp: boolean;
    }
    /**
     * product定制产物输出配置
     */
    interface OutputOpt {
        artifactName: string;
    }
}

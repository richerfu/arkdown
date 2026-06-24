/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/options/configure/app-json-options.d.ts
 */
import { Option } from '../options';
import { AppEnvironmentOption } from './app_environment_option';
import AppEnvironmentObj = AppEnvironmentOption.AppEnvironmentObj;
export declare namespace AppJson {
    interface DeviceConfigurationObj extends Option {
        minAPIVersion?: number;
        distributedNotificationEnabled?: boolean;
        keepAlive?: boolean;
        removable?: boolean;
        singleton?: boolean;
        userDataClearable?: boolean;
        accessible?: boolean;
    }
    interface AppObj extends Option {
        debug?: boolean;
        bundleName: string;
        vendor?: string;
        bundleType?: string;
        versionCode: number;
        versionName: string;
        buildVersion?: string;
        minCompatibleVersionCode?: number;
        minAPIVersion?: number | string;
        targetAPIVersion?: number | string;
        apiReleaseType?: string;
        icon?: string;
        label?: string;
        distributedNotificationEnabled?: boolean;
        singleUser?: boolean;
        description?: string;
        entityType?: string;
        keepAlive?: boolean;
        removable?: boolean;
        singleton?: boolean;
        userDataClearable?: boolean;
        accessible?: boolean;
        multiProjects?: boolean;
        targetBundleList?: string[];
        default?: DeviceConfigurationObj;
        tablet?: DeviceConfigurationObj;
        tv?: DeviceConfigurationObj;
        car?: DeviceConfigurationObj;
        wearable?: DeviceConfigurationObj;
        router?: DeviceConfigurationObj;
        asanEnabled?: boolean;
        tsanEnabled?: boolean;
        hwasanEnabled?: boolean;
        ubsanEnabled?: boolean;
        compileSdkVersion?: string;
        appEnvironments?: AppEnvironmentObj[];
        configuration?: string;
        targetMinorAPIVersion?: number;
        targetPatchAPIVersion?: number;
    }
    interface AppOptObj {
        app: AppObj;
    }
}

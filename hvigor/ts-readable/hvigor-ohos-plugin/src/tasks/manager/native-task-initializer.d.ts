/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/manager/native-task-initializer.d.ts
 */
import { TaskDetails } from '@ohos/hvigor';
import { BuildNativeWithCmake } from '../build-native-with-cmake';
import { BuildNativeWithNinja } from '../build-native-with-ninja';
import { CacheNativeLibs } from '../cache-native-libs';
import { ConfigureCmake } from '../configure-cmake';
import { DoNativeBinxo } from '../do-native-binxo';
import { DoNativeStrip } from '../do-native-strip';
import { PrepareQuickfix } from '../prepare-quickfix';
import { ProcessLibs } from '../process-libs';
import { TargetTaskService } from '../service/target-task-service';
import { TargetTaskCreator, TaskCreatorManager } from '../task-creator';
export declare const initNativeTasks: (creatorManager: TaskCreatorManager, service: TargetTaskService, isFa?: boolean) => void;
export declare class CmakeCA extends TargetTaskCreator {
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
    provider: () => BuildNativeWithCmake;
}
export declare class NinjaCA extends TargetTaskCreator {
    provider: () => BuildNativeWithNinja;
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
}
export declare class ProcessLibsCA extends TargetTaskCreator {
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
    provider: () => ProcessLibs;
}
export declare class DoNativeBinxoCA extends TargetTaskCreator {
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
    provider: () => DoNativeBinxo;
}
export declare class PrepareQuickfixCA extends TargetTaskCreator {
    declareDepends: () => [string, string];
    declareTaskDetail: () => TaskDetails;
    provider: () => PrepareQuickfix;
}
export declare class DoNativeStripCA extends TargetTaskCreator {
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
    provider: () => DoNativeStrip;
}
export declare class CacheNativeLibsCA extends TargetTaskCreator {
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
    provider: () => CacheNativeLibs;
}
export declare class CmakeConfigureCA extends TargetTaskCreator {
    declareDepends: () => string[];
    declareTaskDetail: () => TaskDetails;
    provider: () => ConfigureCmake;
}

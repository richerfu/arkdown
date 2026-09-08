declare const require: (specifier: string) => { writeFileSync(path: string, contents: string): void };
declare const process: { env: Record<string, string | undefined> };

const fs = require("fs");

type ModuleInfo = {
  id: string;
  code?: string | null;
  importedIds?: string[];
};

const tracePath = process.env.ARKDOWN_PLUGIN_TRACE;
if (!tracePath) {
  throw new Error("ARKDOWN_PLUGIN_TRACE is required");
}

const state = {
  events: [] as string[],
  contextKeys: [] as string[],
  buildStartArgumentCount: -1,
  beforeBuildEndArgumentCount: -1,
  buildEndArgumentCount: -1,
  resolveIdCount: 0,
  loadCount: 0,
  transformCount: 0,
  moduleParsedCount: 0,
  shouldInvalidCacheCount: 0,
  cacheRequest: undefined as undefined | { id?: string; code?: string; astType?: string },
  moduleIds: [] as string[],
  moduleInfoCount: 0,
  parsedType: "",
  projectConfig: undefined as unknown,
  projectConfigShape: undefined as unknown,
  depInfo: undefined as unknown,
  hookEventFactoryEnabled: false,
  sourceFileCount: 0,
  sourceHash: undefined as string | undefined,
  sourceMtimeMs: undefined as number | undefined,
  shareCacheBuildCount: 0,
  importPackageName: undefined as string | undefined,
  buildEndError: undefined as undefined | string,
};

export default {
  name: "arkdown-latest-hvigor-lifecycle-fixture",

  buildStart(this: any) {
    state.events.push("buildStart");
    state.buildStartArgumentCount = arguments.length;
    state.contextKeys = Object.keys(this).sort();
    state.projectConfig = this.share?.projectConfig;
    state.projectConfigShape = {
      depName2DepInfoIsMap: this.share.projectConfig.depName2DepInfo instanceof Map,
      dependencyAliasMapIsMap: this.share.projectConfig.dependencyAliasMap instanceof Map,
      permissionRequestCount: this.share.projectConfig.permission?.requestPermissions?.length ?? -1,
      permissionDefineCount: this.share.projectConfig.permission?.definePermissions?.length ?? -1,
      targetESVersion: this.share.projectConfig.projectArkOption?.tscConfig?.targetESVersion,
      maxFlowDepth: this.share.projectConfig.projectArkOption?.tscConfig?.maxFlowDepth,
      bundledDeclare: this.share.projectConfig.projectArkOption?.bundle?.bundledDeclare,
    };
    state.depInfo = this.share.depInfo;
    const hookEventFactory = this.share.getHookEventFactory("fixture", "buildStart");
    state.hookEventFactoryEnabled = hookEventFactory !== undefined;
    const hookEvent = hookEventFactory?.createEvent("fixtureBuildStart");
    hookEvent?.start();
    const hookSubEvent = hookEvent?.createSubEvent("fixtureContext");
    hookSubEvent?.startAsyncEvent();
    hookSubEvent?.stopAsyncEvent("success");
    hookEvent?.stop("success");
    hookEvent?.setTotalTime(1);
    const sourceFiles: string[] = this.share.getSourceFiles();
    const entry = sourceFiles.find((file) => file.endsWith("/Index.ets"));
    state.sourceFileCount = sourceFiles.length;
    state.sourceHash = entry ? this.share.getHashByFilePath(entry) : undefined;
    state.sourceMtimeMs = entry ? this.share.getCachedFileInfo(entry)?.mtimeMs : undefined;
    state.shareCacheBuildCount = (this.share.cache.get("buildCount") ?? 0) + 1;
    this.share.cache.set("buildCount", state.shareCacheBuildCount);
    state.importPackageName = this.share.importResolver(this.share.projectConfig.moduleName)?.packageName;
  },

  resolveId() {
    state.resolveIdCount++;
    return null;
  },

  load() {
    state.loadCount++;
    return null;
  },

  transform(_code: string, _id: string) {
    state.transformCount++;
    if (process.env.ARKDOWN_PLUGIN_THROW_TRANSFORM === "1") {
      throw new Error("intentional transform failure");
    }
    return null;
  },

  shouldInvalidCache(request: { id?: string; code?: string; ast?: { type?: string } }) {
    state.shouldInvalidCacheCount++;
    state.cacheRequest = {
      id: request.id,
      code: request.code,
      astType: request.ast?.type,
    };
    return process.env.ARKDOWN_PLUGIN_FORCE_INVALIDATE === "1";
  },

  moduleParsed(info: ModuleInfo) {
    if (!info || typeof info.id !== "string") {
      throw new Error("moduleParsed did not receive ModuleInfo");
    }
    state.moduleParsedCount++;
  },

  beforeBuildEnd(this: any) {
    state.events.push("beforeBuildEnd");
    state.beforeBuildEndArgumentCount = arguments.length;
    state.moduleIds = Array.from(this.getModuleIds());
    state.moduleInfoCount = state.moduleIds.filter(
      (id) => this.getModuleInfo(id) !== null,
    ).length;
    state.parsedType = this.parse("export const parsed = true").type;
  },

  buildEnd(this: any, error?: Error) {
    state.events.push("buildEnd");
    state.buildEndArgumentCount = arguments.length;
    state.buildEndError = error?.message;
    fs.writeFileSync(tracePath, JSON.stringify(state, null, 2));
  },
};

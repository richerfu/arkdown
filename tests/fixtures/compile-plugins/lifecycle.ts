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
  buildEndError: undefined as undefined | string,
};

export default {
  name: "arkdown-latest-hvigor-lifecycle-fixture",

  buildStart(this: any) {
    state.events.push("buildStart");
    state.buildStartArgumentCount = arguments.length;
    state.projectConfig = this.share?.projectConfig;
  },

  resolveId() {
    state.resolveIdCount++;
    return null;
  },

  load() {
    state.loadCount++;
    return null;
  },

  transform() {
    state.transformCount++;
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

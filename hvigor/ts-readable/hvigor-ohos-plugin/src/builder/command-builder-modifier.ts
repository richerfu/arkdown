/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/builder/command-builder-modifier.js
 */
export var CommandBuilderType;
export function registryCommandModifier(i, e) {
  CommandBuilderModifier.registerModifier(i, e);
}
(function (i) {
  i[i.NONE = 0] = "NONE", i[i.CMAKE = 1] = "CMAKE", i[i.NINJA = 2] = "NINJA";
})(CommandBuilderType = CommandBuilderType || (CommandBuilderType = {}));
export class CommandBuilderModifier {
  constructor(i, e) {
    this.cmdBuilderType = CommandBuilderType.NONE, this.commandBuilder = i, this.cmdBuilderType = e;
  }
  static registerModifier(i, e) {
    CommandBuilderModifier.modifiers.set(i, e);
  }
  wrapCommandBuilder() {
    const i = this.commandBuilder.build.bind(this.commandBuilder);
    return this.commandBuilder.build = () => {
      const e = CommandBuilderModifier.modifiers.get(this.cmdBuilderType);
      let d = i();
      return e && (d = e.modify(d)), d;
    }, this.commandBuilder;
  }
}
CommandBuilderModifier.modifiers = new Map();

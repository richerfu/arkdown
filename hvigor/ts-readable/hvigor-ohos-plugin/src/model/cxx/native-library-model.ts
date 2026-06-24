/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/cxx/native-library-model.js
 */
export class NativeLibraryModel {
  constructor(t, e, i) {
    this._id = e, this._artifactName = t, this._type = i;
  }
  get id() {
    return this._id;
  }
  set id(t) {
    this._id = t;
  }
  getType() {
    return this._type;
  }
  getBuildCommand() {
    return this._buildCommand;
  }
  setBuildCommand(t) {
    this._buildCommand = t;
  }
  getBuildCommandComponents() {
    return this._buildCommandComponents;
  }
  setBuildCommandComponents(t) {
    this._buildCommandComponents = t;
  }
  getBuildType() {
    return this._buildType;
  }
  setBuildType(t) {
    this._buildType = t;
  }
  getToolchain() {
    return this._toolchain;
  }
  setToolchain(t) {
    this._toolchain = t;
  }
  getGroupName() {
    return this._groupName;
  }
  setGroupName(t) {
    this._groupName = t;
  }
  getAbi() {
    return this._abi;
  }
  setAbi(t) {
    this._abi = t;
  }
  getArtifactName() {
    return this._artifactName;
  }
  setArtifactName(t) {
    this._artifactName = t;
  }
  getFolders() {
    return this._folders;
  }
  setFolders(t) {
    this._folders = t;
  }
  getFiles() {
    return this._files;
  }
  setFiles(t) {
    this._files = t;
  }
  getHeaders() {
    return this._headers;
  }
  setHeaders(t) {
    this._headers = t;
  }
  getOutputs() {
    var t;
    return null !== (t = this._outputs) && undefined !== t ? t : [];
  }
  setOutputs(t) {
    this._outputs = t;
  }
  getRuntimeFiles() {
    var t;
    return null !== (t = this._runtimeFiles) && undefined !== t ? t : [];
  }
  setRuntimeFiles(t) {
    this._runtimeFiles = t;
  }
  getCodemodel() {
    return this._codemodel;
  }
  setCodemodel(t) {
    this._codemodel = t;
  }
  getNameOnDisk() {
    return this._nameOnDisk;
  }
  setNameOnDisk(t) {
    this._nameOnDisk = t;
  }
  getBuild() {
    return this._build;
  }
  setBuild(t) {
    this._build = t;
  }
  getSource() {
    return this._source;
  }
  setSource(t) {
    this._source = t;
  }
}

// standard-version updater for src-tauri/Cargo.toml and src-tauri/Cargo.lock.
// Only touches the version of the lv1-offline-patcher package, not dependencies.
const PKG = /(name = "lv1-offline-patcher"\r?\nversion = ")([^"]+)(")/;

module.exports.readVersion = (contents) => contents.match(PKG)[2];
module.exports.writeVersion = (contents, version) => contents.replace(PKG, `$1${version}$3`);

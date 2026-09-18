{ pkgs }:
pkgs.rustPlatform.buildRustPackage {
  pname = "ziral-records";
  version = "0.1.0";
  src = pkgs.lib.fileset.toSource {
    root = ./records;
    fileset = pkgs.lib.fileset.unions [
      ./records/Cargo.toml
      ./records/Cargo.lock
      ./records/src
      ./records/tests
    ];
  };
  cargoLock.lockFile = ./records/Cargo.lock;
}

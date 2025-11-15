{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
      crane,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        craneLib = crane.mkLib pkgs;
        defaultPackage = craneLib.buildPackage {
          src = craneLib.cleanCargoSource ./.;
        };
      in
      {
        devShells.default =
          with pkgs;
          mkShell {
            buildInputs = [
              (rust-bin.stable.latest.default.override {
                extensions = [ "rust-src" ];
              })
              awscli2
              minio-client
            ];
          };
        packages.default = defaultPackage;
        packages.test = pkgs.testers.runNixOSTest {
          name = "test";
          nodes = {
            uploader =
              { config, pkgs, ... }:
              {
                environment.systemPackages = with pkgs; [
                  defaultPackage
                ];
                boot.supportedFilesystems = [
                  "zfs"
                ];
                networking.hostId = "41a604ee";
              };
            server =
              { config, pkgs, ... }:
              {
                services.minio.enable = true;
                networking.firewall.allowedTCPPorts = [ 9000 ];
                environment.systemPackages = with pkgs; [
                  minio-client
                ];
              };
            downloader =
              { config, pkgs, ... }:
              {
                environment.systemPackages = with pkgs; [
                  minio-client
                ];
                boot.supportedFilesystems = [
                  "zfs"
                ];
                networking.hostId = "84bee99e";
              };
          };
          testScript = builtins.readFile ./test.py;
        };
      }
    );
}

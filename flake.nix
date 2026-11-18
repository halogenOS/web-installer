# SPDX-FileCopyrightText: The halogenOS Project
# SPDX-License-Identifier: GPL-3.0-only
{
  description = "Install a release on a phone from the browser, building the images that contain proprietary files on the way";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
  inputs.website = {
    url = "github:halogenOS/website-2026";
    flake = false;
  };

  outputs = { self, nixpkgs, website }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forEverySystem = nixpkgs.lib.genAttrs systems;
      version =
        (builtins.fromTOML (builtins.readFile ./Cargo.toml)).workspace.package.version;
      mkPkgs = system: import nixpkgs { inherit system; };
      wasmTarget = "wasm32-unknown-unknown";
      workspaceArgs = {
        pname = "web-installer";
        inherit version;
        src = ./.;
        cargoLock = {
          lockFile = ./Cargo.lock;
          outputHashes = {
            "byte-delta-1.0.0" = "sha256-NKfYJJGYccdKJ76OFX2VPvpKrHGRswCGrjNw5D0fNxw=";
            "release-0.1.0" = "sha256-bl5j/KSuqwaZdyu17oxJXA1M9kjRRBc95PLzkxa3+/E=";
          };
        };
      };
      wasmBindgenCliVersion = "0.2.130";
      mkWasmBindgenCli = pkgs:
        let
          src = pkgs.fetchCrate {
            pname = "wasm-bindgen-cli";
            version = wasmBindgenCliVersion;
            hash = "sha256-9Og1O/6/VC3sc5hBjoIWBErmLW2UHGMsTQO+6+m/dng=";
          };
        in
        pkgs.buildWasmBindgenCli {
          inherit src;
          cargoDeps = pkgs.rustPlatform.fetchCargoVendor {
            inherit (src) pname version;
            inherit src;
            hash = "sha256-j81sUf1m5yXU4FJfGj7GV+hdqVCV8mRmNb0QIsMCjqw=";
          };
        };
      mkNpmDeps = pkgs: pkgs.fetchNpmDeps {
        name = "web-installer-site-npm-deps";
        src = ./site;
        hash = "sha256-4XVHNGJl/mP2LKK3L6MCCYkBnvZ1XHc9zkpyDsnwPoU=";
      };
      mkSiteWasm = pkgs: pkgs.rustPlatform.buildRustPackage (workspaceArgs // {
        pname = "web-installer-wasm";
        cargoBuildFlags = [ "-p" "wasm" "--target" wasmTarget ];
        doCheck = false;
        nativeBuildInputs = [ (mkWasmBindgenCli pkgs) pkgs.lld ];
        installPhase = ''
          runHook preInstall
          mkdir -p $out
          wasm-bindgen \
            --target web \
            --no-typescript \
            --out-dir $out \
            --out-name web_installer_wasm \
            target/${wasmTarget}/release/wasm.wasm
          runHook postInstall
        '';
      });
      mkSiteBuild = pkgs: { script, ... }@package: pkgs.buildNpmPackage (removeAttrs package [ "script" ] // {
        inherit version;
        src = ./.;
        npmDeps = mkNpmDeps pkgs;
        npmRoot = "site";
        postUnpack = ''
          rm -rf "$sourceRoot/website"
          cp -r ${website} "$sourceRoot/website"
        '';
        preBuild = ''
          cp -r ${mkSiteWasm pkgs}/. site/src/wasm/
        '';
        buildPhase = ''
          runHook preBuild
          pushd site
          npm run ${script}
          popd
          runHook postBuild
        '';
      });
      mkSiteCheck = pkgs: script: mkSiteBuild pkgs {
        pname = "web-installer-site-${script}";
        inherit script;
        installPhase = ''
          runHook preInstall
          touch $out
          runHook postInstall
        '';
      };
    in
    {
      packages = forEverySystem (system:
        let
          pkgs = mkPkgs system;
        in
        rec {
          web-installer = mkSiteBuild pkgs {
            pname = "web-installer";
            script = "build";
            nativeBuildInputs = [ pkgs.makeWrapper ];
            installPhase = ''
              runHook preInstall
              mkdir -p $out/share/web-installer
              cp -r site/.output/. LICENSE $out/share/web-installer/
              makeWrapper ${pkgs.lib.getExe pkgs.nodejs} $out/bin/web-installer \
                --add-flags $out/share/web-installer/server/index.mjs
              runHook postInstall
            '';
            meta = {
              description = "The installer page and the server that delivers it with the releases of halogenOS/releases";
              mainProgram = "web-installer";
            };
          };
          default = web-installer;
        });

      apps = forEverySystem (system:
        let
          pkgs = mkPkgs system;
          testRelease = pkgs.writeShellApplication {
            name = "web-installer-test-release";
            text = ''
              port=8090
              if [ "''${1-}" = --port ]; then
                port="''${2:?--port needs a port}"
                shift 2
              fi
              if [ "$#" -eq 0 ]; then
                echo "Usage: test-release [--port <port>] <dist package folder>..." >&2
                exit 2
              fi
              NITRO_HOST=localhost NITRO_PORT="$port" exec ${pkgs.lib.getExe self.packages.${system}.web-installer} "$@"
            '';
          };
        in
        {
          test-release = {
            type = "app";
            program = pkgs.lib.getExe testRelease;
            meta.description = "Serve the site on this machine with the given dist packages as its only releases, for a test of a build";
          };
        });

      devShells = forEverySystem (system:
        let
          pkgs = mkPkgs system;
        in {
          default = pkgs.mkShell {
            packages = [
              pkgs.cargo
              pkgs.rustc
              pkgs.clippy
              pkgs.rustfmt
              pkgs.lld
              pkgs.nodejs
              (mkWasmBindgenCli pkgs)
            ];
          };
        });

      checks = forEverySystem (system:
        let
          pkgs = mkPkgs system;
        in
        {
          test = pkgs.rustPlatform.buildRustPackage (workspaceArgs // {
            cargoTestFlags = [ "--workspace" ];
          });
          clippy = pkgs.rustPlatform.buildRustPackage (workspaceArgs // {
            pname = "web-installer-clippy";
            nativeBuildInputs = [ pkgs.clippy ];
            buildPhase = ''
              runHook preBuild
              cargo clippy --workspace --all-targets --offline -- -D warnings
              runHook postBuild
            '';
            doCheck = false;
            installPhase = ''
              runHook preInstall
              touch $out
              runHook postInstall
            '';
          });
          site-lint = mkSiteCheck pkgs "lint";
          site-test = mkSiteCheck pkgs "test";
        });
    };
}

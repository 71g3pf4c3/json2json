{
  description = "json2json — lossless JSON to JSON transformer";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems =
        f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAllSystems (
        pkgs:
        let
          cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
        in
        rec {
          json2json = pkgs.rustPlatform.buildRustPackage {
            pname = "json2json";
            inherit (cargoToml.package) version;
            src = self;
            cargoLock.lockFile = ./Cargo.lock;

            # zero dependencies — nothing to vendor, but keep the hook happy
            # with an empty vendor dir anyway
            postInstall = ''
              install -Dm644 README.md $out/share/doc/json2json/README.md
            '';

            meta = {
              description = "Lossless JSON to JSON transformer";
              homepage = "https://github.com/71g3pf4c3/json2json";
              license = pkgs.lib.licenses.gpl3Plus;
              mainProgram = "json2json";
            };
          };
          default = json2json;
        }
      );

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          packages = with pkgs; [
            cargo
            rustc
            rustfmt
            clippy
            rust-analyzer
            jq # for poking at test fixtures
          ];
          RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
        };
      });

      overlays.default = final: _prev: {
        json2json = self.packages.${final.system}.default;
      };

      nixosModules.default = import ./nix/nixos-module.nix self;

      homeManagerModules.default = import ./nix/home-manager-module.nix self;
    };
}

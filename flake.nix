{
  description = "WoW Souls: a single-player souls-like prototype in the World of Warcraft 1.12.1 world (Rust + Bevy)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" ];

      # This repository is the game's source, so the flake builds its own tree:
      # `nix build github:jackke88/wow-souls` builds exactly that revision, with
      # everything .gitignore keeps out (the player's game folders, captures).
      src = self.outPath;

      # The version is the workspace's, so the package cannot lag behind it.
      version = (nixpkgs.lib.importTOML "${src}/Cargo.toml").workspace.package.version;

      pkgsFor =
        system:
        import nixpkgs {
          inherit system;
          overlays = [ self.overlays.default ];
        };

      forSystems = f: nixpkgs.lib.genAttrs systems (system: f (pkgsFor system));
    in
    {
      # `nix fmt` formats the Nix files here in the usual nixpkgs style.
      formatter = forSystems (pkgs: pkgs.nixfmt-rfc-style);

      # `pkgs.wow-souls` in any nixpkgs that gets this overlay.
      overlays.default = final: prev: {
        wow-souls = final.callPackage ./nix/package.nix { inherit src version; };
      };

      packages = forSystems (pkgs: {
        default = pkgs.wow-souls;
        wow-souls = pkgs.wow-souls;
        # The two binaries before wrapping, for building either one on its own.
        game = pkgs.wow-souls.game;
        wow-souls-dsr = pkgs.wow-souls.dsr;
      });

      # `nix run github:jackke88/wow-souls` starts the game.
      apps = forSystems (pkgs: {
        default = {
          type = "app";
          program = "${pkgs.wow-souls}/bin/wow-souls";
        };
        wow-souls = {
          type = "app";
          program = "${pkgs.wow-souls}/bin/wow-souls";
        };
      });

      # NixOS module: `programs.wow-souls.enable = true;`
      # It brings its own overlay, so no separate nixpkgs.overlays entry is needed.
      nixosModules.default = { pkgs, ... }: {
        nixpkgs.overlays = [ self.overlays.default ];
        imports = [ ./nix/nixos-module.nix ];
      };

      # `nix flake check` evaluates the module the way NixOS does: its option types,
      # the merge, and the package override it performs with the configured folders.
      # A mistyped option or a renamed wrapper argument fails here, not on a machine.
      checks = forSystems (pkgs: {
        module =
          let
            # The module writes environment.systemPackages and warnings; declaring
            # those two is enough to evaluate it outside a whole NixOS system.
            evaluated = nixpkgs.lib.evalModules {
              specialArgs = { inherit pkgs; };
              modules = [
                {
                  options.environment.systemPackages = nixpkgs.lib.mkOption {
                    type = nixpkgs.lib.types.listOf nixpkgs.lib.types.package;
                    default = [ ];
                  };
                  options.warnings = nixpkgs.lib.mkOption {
                    type = nixpkgs.lib.types.listOf nixpkgs.lib.types.str;
                    default = [ ];
                  };
                }
                ./nix/nixos-module.nix
                {
                  programs.wow-souls = {
                    enable = true;
                    wowData = "/home/alice/games/WoW_1.12.1";
                    dsrRoot = "/home/alice/.steam/steam/steamapps/common/Dark Souls Remastered";
                    savesDir = "/home/alice/.local/share/wow-souls";
                    extraEnv.WOW_AO = "0";
                  };
                }
              ];
            };
          in
          assert evaluated.config.warnings == [ ];
          pkgs.symlinkJoin {
            name = "wow-souls-module-check";
            paths = evaluated.config.environment.systemPackages;
          };
      });

      devShells = forSystems (pkgs: {
        # For hacking on the game: cargo, rustc and the libraries the build links
        # against, with the runtime libraries on LD_LIBRARY_PATH.
        default = pkgs.mkShell {
          packages = [
            pkgs.cargo
            pkgs.rustc
            pkgs.pkg-config
            pkgs.clippy
            pkgs.rustfmt
          ]
          ++ pkgs.wow-souls.nativeDeps;
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath pkgs.wow-souls.nativeDeps;
          WOWSOULS_DATA_DIR = "${pkgs.wow-souls}/share/wow-souls";
        };
      });
    };
}

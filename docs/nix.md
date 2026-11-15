# WoW Souls on Nix and NixOS

The repository is a Nix flake: it builds a native Linux binary of the game and of its GPL
animation builder from this tree, wraps them with the environment the game reads, and installs
them system-wide on NixOS through `programs.wow-souls`. Nobody has to read any of this to play on
Windows, where `Play.bat` is still the way in.

| File | What it is |
|---|---|
| `flake.nix` | inputs, overlay, `packages.*`, `apps.*`, `checks.*`, `nixosModules.default`, a dev shell |
| `nix/package.nix` | the two cargo workspaces and the wrapper around them |
| `nix/nixos-module.nix` | the `programs.wow-souls.*` options |
| `docs/nix.md` | this page |

## Try it

```console
nix build '.#wow-souls'
./result/bin/wow-souls
```

`nix run github:jackke88/wow-souls` does the same thing without a clone, and
`nix develop` gives a shell with `cargo`, `rustc` and the libraries the build links against.

A cold build takes 10-15 minutes on eight cores, most of it in the engine and Bevy.

Like the Windows build, this one needs your own World of Warcraft 1.12.1 (build 5875) client and
your own Dark Souls Remastered; neither is in the package or in the Nix store. On the first start
the game asks for each folder in a dialog and remembers it under the state folder,
`$XDG_DATA_HOME/wow-souls/WoWSouls`, exactly as it does on Windows. Passing `WOW_DATA` and
`WOWSOULS_DSR` skips the asking, which is what the NixOS module below does for you.
`wow-souls-dsr` takes the same two folders as `--wow` and `--dsr` if you want to run the
animation builder yourself.

## Install it on NixOS

In your configuration flake:

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    wow-souls.url = "github:jackke88/wow-souls";
  };

  outputs = { self, nixpkgs, wow-souls, ... }: {
    nixosConfigurations.myhost = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        ./configuration.nix
        wow-souls.nixosModules.default
        {
          programs.wow-souls = {
            enable = true;
            wowData = "/home/alice/games/WoW_1.12.1";
            dsrRoot = "/home/alice/.steam/steam/steamapps/common/Dark Souls Remastered";
          };
        }
      ];
    };
  };
}
```

`programs.wow-souls.enable` puts `wow-souls`, `wow-souls-dsr` and
`wow-souls-build-animations` in the system path, and the package carries a `wow-souls.desktop`
entry for menus that index system packages. With `wowData` and `dsrRoot` set the first start opens
a window instead of two dialogs, and the combat animations are built by the wrapper on the first
start; `wow-souls-build-animations` runs the same build by hand, `--force` to build again, and the
log is `WoWSouls/anim-setup.log` under the state folder. Both options stay plain paths, so neither
client is ever copied into the store.

| Option | What it is |
|---|---|
| `enable` | install the game system-wide |
| `package` | the derivation to install; defaults to this flake's `pkgs.wow-souls` |
| `wowData` | your WoW 1.12.1 client: the folder with `WoW.exe`, or its `Data` folder |
| `dsrRoot` | your Dark Souls Remastered install, used to build the combat animations |
| `animDir` | where the built animation library lives (`WOWSOULS_ANIM_DIR`) |
| `savesDir` | state folder for saves, settings and the animation library (`LOCALAPPDATA`) |
| `spokenDir` | root of a Spoken Player voice pack; `"0"` turns quest voices off |
| `extraEnv` | extra variables the wrapper exports, e.g. `{ WOW_AO = "0"; }` |

The module brings its own overlay, so no separate `nixpkgs.overlays` entry is needed. To build
the package outside NixOS with the same settings, override it directly:
`pkgs.wow-souls.override { wowData = ...; dsrRoot = ...; }`.

## What gets built

The repository holds two cargo workspaces and the game needs both:

- `crates/wow-souls` is the game. It is built with `--no-default-features`, which is the player
  build (`dev` adds engine instruments and a developer source path).
- `crates/dsr-cache` is `wow-souls-dsr`, the GPL-3.0 animation builder the game runs as a separate
  program so that its GPL code never links into the game. The game looks for it next to its own
  executable, so the package keeps both under `libexec/wow-souls/` and puts the wrapper in `bin/`.

The package also carries the repository's `data/` (world and quest exports, voice pack inventory)
and `anim/` (animation tables, clip provenance), and points the game at them through
`WOWSOULS_DATA_DIR`.

The wrapper is a small shell script. It is generated at build time, so it names its own store
prefix and works both from the store and from `/run/current-system/sw/bin`. Everything it sets is
a default: a variable you have already exported wins.

| Variable | What the wrapper does with it |
|---|---|
| `WOWSOULS_DATA_DIR` | points at the packaged `data/` and `anim/`; override it to use a writable copy |
| `LOCALAPPDATA` | the state folder, defaulting to `$XDG_DATA_HOME/wow-souls` |
| `WOW_DATA` | your client folder, accepted either as the folder holding `WoW.exe` or as its `Data` folder |
| `WOWSOULS_DSR` | your Dark Souls folder, passed to the animation builder |
| `WOWSOULS_ANIM_DIR` | the built animation library |
| `WOWSOULS_SPOKEN` | a Spoken Player voice pack |
| `LD_LIBRARY_PATH` | ALSA, libudev, EGL, the Wayland and X11 libraries, and the Vulkan loader |

## Things to know

- Linux is not the platform the game is developed on. The code has no Windows-only paths in it
  (Bevy, Avian, cpal and symphonia are all cross-platform), but it is not tested on Linux by the
  developer, so expect rough edges. `nix flake check` is what is verified here.
- The folder dialogs go through the freedesktop portal (`rfd` with `xdg-portal`). Without
  `xdg.portal.enable` there is no dialog to fall back on, so pass `wowData` and `dsrRoot`; the
  module warns at build time when a folder is missing and no portal is enabled to ask for it.
- Rendering needs a Vulkan or GL driver from the system: `hardware.graphics` with mesa for
  AMD/Intel, or the `nixpkgs.config.cuda` setup for NVIDIA. The wrapper adds nixpkgs'
  `vulkan-loader`; the ICD manifest comes from the system driver.
- Quest voices play out of the package: `data/voices` ships 189 Spoken Player recordings.
  `spokenDir` (or `WOWSOULS_SPOKEN`) selects a fuller voice pack instead, and `"0"` turns voices off.
- A gamepad works through evdev; nothing to enable.
- `--audit-westfall` writes its reports into a `docs` folder under `WOWSOULS_DATA_DIR`. The game
  itself never writes there, so only that check stops with `Read-only file system` against the
  packaged copy: point `WOWSOULS_DATA_DIR` at a writable copy of `share/wow-souls` to run it.
- The package builds for `x86_64-linux`. Other systems are not declared and probably need work
  (Bevy needs ALSA, X11 and Wayland libraries at build time).

## Updating the hashes

The two `cargoHash` values in `nix/package.nix` cover the vendored crates of the two workspaces,
`vendor/benilla` included. When `Cargo.lock` changes, one of them no longer matches and nix prints
the value it expected; paste each into `cargoHash` / `dsrCargoHash` in the same commit that changes
the lock file. The version is taken from `[workspace.package] version` in `Cargo.toml`, so nothing
to bump there either.

While hacking on the game, build the working tree rather than the pushed revision:

```console
nix build '.#wow-souls'
```

which is what the flake does by default: its source is the flake's own git tree, so
`.gitignore`d folders (`/WoW`, `target/`, captures) never reach the store.

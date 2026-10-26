# Material UI assets

`etched-icons-v1.png` and `weathered-trim-v1.png` were generated with the built-in image tool from the user's Elden Ring menu screenshot as a style reference. No installed game files were used as generation inputs. These sheets contain only reusable artwork: etched menu icons, normal/selected empty slots, a thin bar frame, neutral fill pigment, an empty level seal and a smoky panel surface. All game text, resource values, layout, navigation and animations are rendered by code.

Original generated RGBA files are preserved without pixel editing. Transparent gutters are checked by the atlas test. The game embeds both files and maps regions in `hud/theme/skin.rs`; `hud/theme.rs` supplies layout primitives and fallbacks. No DSR HUD cache is loaded in the default theme. These small sheets supersede the rejected large bronze class-emblem HUD.

Reference: https://interfaceingame.com/screenshots/elden-ring-menu-in-game/

Exact built-in prompts: [prompts.md](prompts.md).

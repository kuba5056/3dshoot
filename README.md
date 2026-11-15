# 3D Shoot

A first-person shooter prototype built on the
[Jewish Game Engine](https://github.com/kuba5056/Jewish-Game-Engine).

This is the foundation, without enemies yet: walk around an arena, shoot the
glowing targets (they drop away and come back after a few seconds) and knock
the crates and barrels about.

```
cargo run
```

On NixOS run `nix-shell` first. On Debian/Ubuntu install `libasound2-dev`
once. The first build downloads and compiles the engine, which takes a few
minutes; `cargo run --release` gives a smoother game.

## Controls

| | |
|---|---|
| Click / Play | grab the mouse and start |
| Mouse | look (the arrow keys also turn the view) |
| W A S D | move |
| Shift | sprint |
| Space | jump |
| Left mouse button | fire (hold for automatic fire) |
| R | reload |
| Escape | pause and release the mouse |
| F3 | the engine's debug overlay |

## What is here

- `src/main.rs` – the game loop: menu, HUD, targets, wiring.
- `src/player.rs` – the first-person controller: a physics capsule with
  the camera at its eyes, mouse look, walking, sprinting, jumping.
- `src/weapon.rs` – the gun: hit-scan shots, magazine and reload, view
  model, muzzle flash, impact sparks, sounds.
- `assets/scenes/arena.ron` – the level. Entities named `Target` become
  targets and `Start` is where the player appears; everything else is
  scenery. Edit it by hand or with the engine's editor:

  ```
  cargo run --manifest-path ../Jewish-Game-Engine/Cargo.toml -p editor -- scenes/arena.ron
  ```
- `assets/sounds/` – generated sound effects.

Levels are built from boxes, spheres and capsules: the engine has no
collision against arbitrary model meshes.

## Checks

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

The engine's `docs/GUIDE.md` documents the API used here.

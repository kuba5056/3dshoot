//! 3D Shoot: a first-person shooter prototype.
//!
//! Walk around an arena, shoot the glowing targets and knock the crates and
//! barrels about. No enemies yet: this is the foundation (movement, looking,
//! shooting, reloading, targets, a HUD).
//!
//! Controls: click to grab the mouse, mouse to look, WASD to move, Shift to
//! sprint, Space to jump, left button to fire, R to reload, Escape to pause,
//! F3 for the engine's debug overlay. The arrow keys also turn the view.
//!
//! Run with `cargo run` from this directory. `--exit-after <seconds>` quits
//! by itself, for automated runs.

mod player;
mod weapon;

use engine::egui::{self, Align2, Color32, RichText};
use engine::math::Vec3;
use engine::{
    find_by_name, Config, Context, Entity, Game, KeyCode, MouseButton, Transform, WindowConfig,
    World,
};

use player::Player;
use weapon::Weapon;

/// How long a target stays down after being hit, in seconds.
const TARGET_RESPAWN: f32 = 3.0;

/// A plate to shoot. Hit, it drops out of the arena and returns later.
struct Target {
    home: Vec3,
    /// Seconds until it comes back; 0 while it is up.
    down_for: f32,
}

struct Shooter {
    player: Option<Player>,
    weapon: Option<Weapon>,
    /// Whether the mouse is grabbed and the game is live.
    playing: bool,
    /// Whether the game has been started at least once (the menu then
    /// offers "Resume" instead of "Play").
    started: bool,
    hits: u32,
    exit_after: Option<f32>,
}

impl Shooter {
    fn set_playing(&mut self, ctx: &mut Context, playing: bool) {
        self.playing = playing;
        self.started |= playing;
        ctx.set_cursor_captured(playing);
        ctx.physics.paused = !playing;
    }

    /// Something was shot: knock targets down, shove loose objects.
    /// Returns whether a target was scored.
    fn on_hit(&mut self, ctx: &mut Context, entity: Entity, direction: Vec3) -> bool {
        if knock_down(&mut ctx.world, entity) {
            self.hits += 1;
            return true;
        }
        // Does nothing to static scenery, which has no dynamic body.
        ctx.physics.apply_impulse(entity, direction * 2.5);
        false
    }

    fn draw_hud(&self, ctx: &Context) {
        let ui_ctx = ctx.ui();
        // HUD areas are not interactable: otherwise a click with the cursor
        // over one (the crosshair sits right under it) would go to the
        // interface instead of firing.
        let hud = |id: &'static str, anchor: Align2, offset: [f32; 2]| {
            egui::Area::new(egui::Id::unique(id))
                .anchor(anchor, offset)
                .interactable(false)
        };
        let text =
            |s: String, size: f32| RichText::new(s).size(size).color(Color32::WHITE).strong();

        hud("crosshair", Align2::CENTER_CENTER, [0.0, 0.0])
            .show(&ui_ctx, |ui| ui.label(text("+".to_owned(), 26.0)));
        hud("hits", Align2::LEFT_BOTTOM, [16.0, -14.0]).show(&ui_ctx, |ui| {
            ui.label(text(format!("Targets hit: {}", self.hits), 20.0))
        });
        if let Some(weapon) = &self.weapon {
            let ammo = if weapon.is_reloading() {
                "Reloading...".to_owned()
            } else {
                format!("{} / {}", weapon.ammo(), weapon::MAGAZINE)
            };
            hud("ammo", Align2::RIGHT_BOTTOM, [-16.0, -14.0]).show(&ui_ctx, |ui| {
                // Keep it on one line however narrow the corner is.
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                ui.label(text(ammo, 24.0));
            });
        }
    }

    /// The panel shown while the mouse is free: start, resume or quit.
    fn draw_menu(&mut self, ctx: &mut Context) {
        let mut resume = false;
        let mut quit = false;
        egui::Window::new("menu")
            .title_bar(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(&ctx.ui(), |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
                    ui.label(RichText::new("3D Shoot").size(36.0).strong());
                    ui.add_space(4.0);
                    ui.label("Mouse: look   WASD: move   Shift: sprint   Space: jump");
                    ui.label("Left button: fire   R: reload   Esc: pause");
                    ui.add_space(12.0);
                    let label = if self.started { "Resume" } else { "Play" };
                    let button = egui::Button::new(RichText::new(label).size(20.0));
                    resume = ui.add_sized([220.0, 38.0], button).clicked();
                    ui.add_space(6.0);
                    let button = egui::Button::new(RichText::new("Quit").size(20.0));
                    quit = ui.add_sized([220.0, 38.0], button).clicked();
                    ui.add_space(6.0);
                });
            });
        if resume {
            self.set_playing(ctx, true);
        }
        if quit {
            ctx.exit();
        }
    }
}

/// Knocks a target down if `entity` is one that is currently up. A downed
/// target is moved far below the arena (its collider goes with it) until
/// [`restore_targets`] brings it back.
fn knock_down(world: &mut World, entity: Entity) -> bool {
    let Ok((transform, target)) = world.query_one_mut::<(&mut Transform, &mut Target)>(entity)
    else {
        return false;
    };
    if target.down_for > 0.0 {
        return false;
    }
    target.down_for = TARGET_RESPAWN;
    transform.translation.y -= 100.0;
    true
}

/// Counts down every downed target and puts it back when its time is up.
fn restore_targets(world: &mut World, dt: f32) {
    for (transform, target) in world.query_mut::<(&mut Transform, &mut Target)>() {
        if target.down_for > 0.0 {
            target.down_for -= dt;
            if target.down_for <= 0.0 {
                target.down_for = 0.0;
                transform.translation = target.home;
            }
        }
    }
}

impl Game for Shooter {
    fn init(&mut self, ctx: &mut Context) {
        ctx.renderer.clear_color = Vec3::new(0.35, 0.5, 0.75);
        ctx.renderer.shadow_distance = 30.0;
        ctx.camera.fov_y_degrees = 75.0;

        if let Err(e) = ctx.load_scene("scenes/arena.ron") {
            engine::log::error!("{e}");
            ctx.exit();
            return;
        }
        // Scene files mark targets by name; give each its game component.
        for entity in find_by_name(&ctx.world, "Target") {
            let home = ctx.world.get::<&Transform>(entity).map(|t| t.translation);
            if let Ok(home) = home {
                let target = Target {
                    home,
                    down_for: 0.0,
                };
                let _ = ctx.world.insert_one(entity, target);
            }
        }
        let start = find_by_name(&ctx.world, "Start")
            .first()
            .and_then(|&e| ctx.world.get::<&Transform>(e).ok().map(|t| t.translation))
            .unwrap_or(Vec3::Y);

        let player = Player::spawn(ctx, start);
        player.place_camera(ctx, 1.0);
        self.player = Some(player);
        self.weapon = Some(Weapon::new(ctx));
        // Starts on the menu, with the world frozen behind it.
        self.set_playing(ctx, false);
    }

    fn fixed_update(&mut self, ctx: &mut Context) {
        if let (true, Some(player)) = (self.playing, &mut self.player) {
            player.fixed_update(ctx);
        }
    }

    fn update(&mut self, ctx: &mut Context) {
        let elapsed = ctx.time.elapsed().as_secs_f32();
        if self.exit_after.is_some_and(|limit| elapsed >= limit) {
            ctx.exit();
        }
        if ctx.input.key_pressed(KeyCode::Escape) {
            self.set_playing(ctx, !self.playing);
        }

        if self.playing {
            let (Some(player), Some(weapon)) = (&mut self.player, &mut self.weapon) else {
                return;
            };
            player.update(ctx);
            let entity = player.entity();
            let trigger = ctx.input.button_down(MouseButton::Left);
            let reload = ctx.input.key_pressed(KeyCode::KeyR);
            if let Some(shot) = weapon.update(ctx, entity, trigger, reload) {
                player.kick(shot.recoil);
                if let Some((hit, direction)) = shot.hit {
                    let scored = self.on_hit(ctx, hit, direction);
                    if let Some(weapon) = &self.weapon {
                        weapon.play_impact(ctx, scored);
                    }
                }
            }
            restore_targets(&mut ctx.world, ctx.time.delta_secs());
        } else {
            self.draw_menu(ctx);
        }

        // The gun and HUD stay on screen behind the menu too.
        if let Some(weapon) = &self.weapon {
            weapon.draw(ctx);
        }
        self.draw_hud(ctx);
    }
}

fn main() -> Result<(), engine::Error> {
    let mut args = std::env::args().skip(1);
    let mut exit_after = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--exit-after" => exit_after = args.next().and_then(|v| v.parse().ok()),
            other => eprintln!("ignoring unknown option {other}"),
        }
    }
    let config = Config {
        window: WindowConfig {
            title: "3D Shoot".to_owned(),
            ..WindowConfig::default()
        },
        ..Config::default()
    };
    let game = Shooter {
        player: None,
        weapon: None,
        playing: false,
        started: false,
        hits: 0,
        exit_after,
    };
    engine::run(config, game)
}

#[cfg(test)]
mod tests {
    use engine::{Collider, Physics, RigidBody};

    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn spawn_target(world: &mut World, home: Vec3) -> Entity {
        world.spawn((
            Transform::from_translation(home).with_scale(Vec3::new(1.0, 1.0, 0.15)),
            Collider::default(),
            Target {
                home,
                down_for: 0.0,
            },
        ))
    }

    #[test]
    fn a_shot_along_the_view_finds_the_target_and_knocks_it_down() {
        let (mut world, mut physics) = (World::new(), Physics::new());
        let home = Vec3::new(0.0, 2.0, -20.0);
        let target = spawn_target(&mut world, home);
        let shooter = world.spawn((
            Transform::from_xyz(0.0, 2.0, 0.0),
            Collider::capsule(0.5, 0.35),
            RigidBody {
                gravity_scale: 0.0,
                ..RigidBody::dynamic()
            },
        ));
        physics.step(&mut world, DT);

        // The ray starts inside the shooter's own capsule, which is skipped.
        let hit = physics
            .raycast(Vec3::new(0.0, 2.0, 0.0), Vec3::NEG_Z, 200.0, Some(shooter))
            .expect("the target is straight ahead");
        assert_eq!(hit.entity, target);
        assert!((hit.distance - 19.925).abs() < 0.01, "{}", hit.distance);

        assert!(knock_down(&mut world, target));
        physics.step(&mut world, DT);
        // Gone from where it stood: the same shot now passes through.
        let after = physics.raycast(Vec3::new(0.0, 2.0, 0.0), Vec3::NEG_Z, 200.0, Some(shooter));
        assert!(after.is_none());
    }

    #[test]
    fn a_downed_target_cannot_be_scored_again_until_it_returns() {
        let mut world = World::new();
        let home = Vec3::new(3.0, 1.5, -8.0);
        let target = spawn_target(&mut world, home);

        assert!(knock_down(&mut world, target));
        assert!(!knock_down(&mut world, target), "already down");
        assert!(world.get::<&Transform>(target).unwrap().translation.y < -50.0);

        restore_targets(&mut world, TARGET_RESPAWN - 0.1);
        assert!(!knock_down(&mut world, target), "still down");
        restore_targets(&mut world, 0.2);
        assert_eq!(world.get::<&Transform>(target).unwrap().translation, home);
        assert!(knock_down(&mut world, target), "up again");
    }

    #[test]
    fn scenery_is_not_a_target() {
        let mut world = World::new();
        let wall = world.spawn((Transform::IDENTITY, Collider::default()));
        assert!(!knock_down(&mut world, wall));
    }

    #[test]
    fn the_arena_scene_parses_and_has_what_the_game_looks_for() {
        let text = std::fs::read_to_string("assets/scenes/arena.ron").unwrap();
        let scene = engine::SceneFile::from_ron(&text).unwrap();
        let count = |name: &str| {
            scene
                .entities
                .iter()
                .filter(|e| e.name.as_deref() == Some(name))
                .count()
        };
        assert_eq!(count("Start"), 1);
        assert_eq!(count("Target"), 10);
        assert!(count("Crate") > 0 && count("Barrel") > 0);
        // Every target needs a collider, or shots pass through it.
        for entity in scene
            .entities
            .iter()
            .filter(|e| e.name.as_deref() == Some("Target"))
        {
            assert!(entity.collider.is_some());
        }
    }
}

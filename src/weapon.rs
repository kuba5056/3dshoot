//! The gun: hit-scan shots along the view, a magazine with reloading, a
//! simple view model, muzzle flash, impact sparks and sounds.

use engine::math::{Mat4, Vec3, Vec4};
use engine::{
    Context, Entity, Material, Mesh, MeshData, ParticleEmitter, Playback, Sound, Transform,
};

pub const MAGAZINE: u32 = 12;
/// Seconds between shots while the trigger is held.
const FIRE_INTERVAL: f32 = 0.14;
const RELOAD_TIME: f32 = 1.2;
const RANGE: f32 = 200.0;
/// How long the muzzle flash lights the surroundings, in seconds.
const FLASH_TIME: f32 = 0.05;

/// What one shot did.
pub struct Shot {
    /// The entity hit and the direction the bullet travelled.
    pub hit: Option<(Entity, Vec3)>,
    /// How far to jolt the view upwards, in radians.
    pub recoil: f32,
}

#[derive(Default)]
struct Sounds {
    shot: Option<Sound>,
    ping: Option<Sound>,
    impact: Option<Sound>,
    reload: Option<Sound>,
    empty: Option<Sound>,
}

pub struct Weapon {
    ammo: u32,
    cooldown: f32,
    /// Seconds of reload left; 0 when ready.
    reloading: f32,
    /// How far the gun is pushed back by the last shot; eases to 0.
    kick: f32,
    flash: f32,
    /// Whether the trigger was already down on an empty magazine, so the
    /// "click" plays once per pull.
    dry_fired: bool,
    mesh: Mesh,
    sounds: Sounds,
}

impl Weapon {
    pub fn new(ctx: &mut Context) -> Self {
        let mut load = |path: &str| {
            ctx.load_sound(path)
                .map_err(|e| engine::log::warn!("{e}"))
                .ok()
        };
        let sounds = Sounds {
            shot: load("sounds/shot.wav"),
            ping: load("sounds/ping.wav"),
            impact: load("sounds/impact.wav"),
            reload: load("sounds/reload.wav"),
            empty: load("sounds/empty.wav"),
        };
        Self {
            ammo: MAGAZINE,
            cooldown: 0.0,
            reloading: 0.0,
            kick: 0.0,
            flash: 0.0,
            dry_fired: false,
            mesh: ctx.renderer.create_mesh(&MeshData::cube(1.0)),
            sounds,
        }
    }

    pub fn ammo(&self) -> u32 {
        self.ammo
    }

    pub fn is_reloading(&self) -> bool {
        self.reloading > 0.0
    }

    fn play(ctx: &mut Context, sound: &Option<Sound>, volume: f32, pitch: f32) {
        if let Some(sound) = sound {
            let playback = Playback {
                volume,
                pitch,
                ..Playback::default()
            };
            ctx.audio.play_with(sound, playback);
        }
    }

    /// The sound of a bullet landing: a ping for a target, a tick otherwise.
    pub fn play_impact(&self, ctx: &mut Context, on_target: bool) {
        if on_target {
            Self::play(ctx, &self.sounds.ping, 0.7, 1.0);
        } else {
            Self::play(ctx, &self.sounds.impact, 0.5, 1.0);
        }
    }

    fn start_reload(&mut self, ctx: &mut Context) {
        if self.reloading <= 0.0 && self.ammo < MAGAZINE {
            self.reloading = RELOAD_TIME;
            Self::play(ctx, &self.sounds.reload, 0.7, 1.0);
        }
    }

    /// Advances timers and fires if the trigger is held and the gun is
    /// ready. `shooter` is left out of the hit test.
    pub fn update(
        &mut self,
        ctx: &mut Context,
        shooter: Entity,
        trigger: bool,
        reload: bool,
    ) -> Option<Shot> {
        let dt = ctx.time.delta_secs();
        self.cooldown = (self.cooldown - dt).max(0.0);
        self.flash = (self.flash - dt).max(0.0);
        self.kick *= (-14.0 * dt).exp();
        if self.reloading > 0.0 {
            self.reloading -= dt;
            if self.reloading <= 0.0 {
                self.reloading = 0.0;
                self.ammo = MAGAZINE;
            }
            return None;
        }
        if reload {
            self.start_reload(ctx);
            return None;
        }
        if !trigger {
            self.dry_fired = false;
            return None;
        }
        if self.cooldown > 0.0 {
            return None;
        }
        if self.ammo == 0 {
            if !self.dry_fired {
                self.dry_fired = true;
                Self::play(ctx, &self.sounds.empty, 0.6, 1.0);
            }
            return None;
        }

        self.ammo -= 1;
        self.cooldown = FIRE_INTERVAL;
        self.kick = 1.0;
        self.flash = FLASH_TIME;
        // A little pitch variation keeps rapid fire from sounding canned.
        let pitch = 0.94 + 0.12 * ((self.ammo % 5) as f32 / 4.0);
        Self::play(ctx, &self.sounds.shot, 0.6, pitch);

        let (origin, direction) = (ctx.camera.position, ctx.camera.forward());
        let hit = ctx.physics.raycast(origin, direction, RANGE, Some(shooter));
        if let Some(hit) = &hit {
            // Sparks at the point of impact, thrown back off the surface.
            let sparks = ParticleEmitter {
                direction: hit.normal,
                spread: 55.0,
                speed: (1.5, 4.5),
                lifetime: (0.15, 0.4),
                size: (0.07, 0.01),
                color_start: Vec4::new(4.0, 2.6, 0.8, 1.0),
                color_end: Vec4::new(2.5, 0.6, 0.1, 0.0),
                ..ParticleEmitter::burst(14)
            };
            ctx.world
                .spawn((Transform::from_translation(hit.point), sparks));
        }
        Some(Shot {
            hit: hit.map(|hit| (hit.entity, direction)),
            recoil: 0.018,
        })
    }

    /// Draws the gun in the lower right of the view, and its muzzle flash.
    pub fn draw(&self, ctx: &mut Context) {
        let camera = &ctx.camera;
        let (forward, right, up) = (camera.forward(), camera.right(), camera.up());
        // Lowered while reloading, pushed back by recoil.
        let lowered = if self.reloading > 0.0 { 0.12 } else { 0.0 };
        let centre = camera.position + right * 0.17 - up * (0.15 + lowered)
            + forward * (0.42 - self.kick * 0.05);

        let metal = Material {
            metallic: 1.0,
            roughness: 0.45,
            ..Material::colored(0.12, 0.12, 0.14)
        };
        let part = |offset: Vec3, size: Vec3| {
            Mat4::from_scale_rotation_translation(size, camera.rotation, centre + offset)
        };
        let barrel = part(Vec3::ZERO, Vec3::new(0.035, 0.045, 0.26));
        let grip = part(-up * 0.06 - forward * 0.08, Vec3::new(0.03, 0.09, 0.045));
        // A helper, not part of the world: it should not cast a shadow.
        ctx.renderer.draw_without_shadow(&self.mesh, &metal, barrel);
        ctx.renderer.draw_without_shadow(&self.mesh, &metal, grip);

        if self.flash > 0.0 {
            let muzzle = centre + forward * 0.17;
            let strength = self.flash / FLASH_TIME;
            ctx.renderer
                .add_point_light(muzzle, Vec3::new(1.0, 0.7, 0.3), 6.0 * strength, 9.0);
            let glow = Material {
                base_color: Vec4::new(0.0, 0.0, 0.0, 1.0),
                emissive: Vec3::new(6.0, 3.5, 1.0) * strength,
                ..Material::default()
            };
            let flash =
                Mat4::from_scale_rotation_translation(Vec3::splat(0.09), camera.rotation, muzzle);
            ctx.renderer.draw_without_shadow(&self.mesh, &glow, flash);
        }
    }
}

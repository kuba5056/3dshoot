//! The first-person player: a capsule moved by the physics engine, with the
//! camera at its eyes.

use engine::math::Vec3;
use engine::{Collider, Context, Entity, KeyCode, Name, RigidBody, Transform};

/// Capsule size: a cylinder of this half-height with rounded ends.
const HALF_HEIGHT: f32 = 0.5;
const RADIUS: f32 = 0.35;
/// Height of the eyes above the capsule's centre.
const EYE_HEIGHT: f32 = 0.65;

const WALK_SPEED: f32 = 6.0;
const SPRINT_SPEED: f32 = 9.5;
const JUMP_SPEED: f32 = 6.5;
/// How quickly velocity follows the keys, per second: snappy on the ground,
/// only a little steering in the air.
const GROUND_CONTROL: f32 = 14.0;
const AIR_CONTROL: f32 = 2.5;
/// A jump pressed slightly before landing still counts.
const JUMP_BUFFER: f32 = 0.15;

/// Radians of turn per pixel of mouse movement, and per second of arrow key.
const MOUSE_SENSITIVITY: f32 = 0.0025;
const KEY_TURN_SPEED: f32 = 2.0;
/// Just short of straight up and down, where "left" and "right" lose meaning.
const MAX_PITCH: f32 = 1.5;

pub struct Player {
    entity: Entity,
    yaw: f32,
    pitch: f32,
    /// Extra upward pitch from firing; decays back to zero.
    recoil: f32,
    jump_buffer: f32,
}

impl Player {
    pub fn spawn(ctx: &mut Context, position: Vec3) -> Self {
        let entity = ctx.world.spawn((
            Name("Player".to_owned()),
            Transform::from_translation(position),
            // No friction, so the capsule slides along walls instead of
            // sticking to them; stopping is handled by the movement code.
            Collider::capsule(HALF_HEIGHT, RADIUS).with_friction(0.0),
            RigidBody {
                lock_rotation: true,
                ..RigidBody::dynamic()
            },
        ));
        Self {
            entity,
            yaw: 0.0,
            pitch: 0.0,
            recoil: 0.0,
            jump_buffer: 0.0,
        }
    }

    pub fn entity(&self) -> Entity {
        self.entity
    }

    /// Jolts the view upwards by `amount` radians.
    pub fn kick(&mut self, amount: f32) {
        self.recoil += amount;
    }

    fn position(&self, ctx: &Context) -> Option<Vec3> {
        let transform = ctx.world.get::<&Transform>(self.entity).ok()?;
        Some(transform.translation)
    }

    /// Movement, in step with the physics.
    pub fn fixed_update(&mut self, ctx: &mut Context) {
        let dt = ctx.time.fixed_delta_secs();
        let Some(position) = self.position(ctx) else {
            return;
        };
        // `None` until the first physics step has created the body.
        let Some(mut velocity) = ctx.physics.linear_velocity(self.entity) else {
            return;
        };

        // Directions on the ground plane for the way the player faces.
        let forward = Vec3::new(-self.yaw.sin(), 0.0, -self.yaw.cos());
        let right = Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin());
        let axis = |positive: KeyCode, negative: KeyCode| {
            ctx.input.key_down(positive) as i32 as f32 - ctx.input.key_down(negative) as i32 as f32
        };
        let wish = (forward * axis(KeyCode::KeyW, KeyCode::KeyS)
            + right * axis(KeyCode::KeyD, KeyCode::KeyA))
        .normalize_or_zero();
        let speed = if ctx.input.key_down(KeyCode::ShiftLeft) {
            SPRINT_SPEED
        } else {
            WALK_SPEED
        };

        // A short ray from the capsule's centre to just below its feet.
        let reach = HALF_HEIGHT + RADIUS + 0.1;
        let grounded = ctx
            .physics
            .raycast(position, Vec3::NEG_Y, reach, Some(self.entity))
            .is_some();

        let control = if grounded {
            GROUND_CONTROL
        } else {
            AIR_CONTROL
        };
        let target = wish * speed;
        let blend = (control * dt).min(1.0);
        velocity.x += (target.x - velocity.x) * blend;
        velocity.z += (target.z - velocity.z) * blend;

        self.jump_buffer = (self.jump_buffer - dt).max(0.0);
        if grounded && self.jump_buffer > 0.0 {
            velocity.y = JUMP_SPEED;
            self.jump_buffer = 0.0;
        }
        ctx.physics.set_linear_velocity(self.entity, velocity);

        // Fell out of the arena somehow: back to the middle.
        if position.y < -20.0 {
            if let Ok(mut transform) = ctx.world.get::<&mut Transform>(self.entity) {
                transform.translation = Vec3::new(0.0, 2.0, 0.0);
            }
            ctx.physics.set_linear_velocity(self.entity, Vec3::ZERO);
        }
    }

    /// Looking around and the camera, once per frame.
    pub fn update(&mut self, ctx: &mut Context) {
        let dt = ctx.time.delta_secs();
        // One-off presses are read here: fixed_update may run zero or
        // several times in a frame and would miss or repeat them.
        if ctx.input.key_pressed(KeyCode::Space) {
            self.jump_buffer = JUMP_BUFFER;
        }

        let (dx, dy) = ctx.input.mouse_delta();
        let key = |positive: KeyCode, negative: KeyCode| {
            ctx.input.key_down(positive) as i32 as f32 - ctx.input.key_down(negative) as i32 as f32
        };
        self.yaw -= dx * MOUSE_SENSITIVITY;
        self.yaw += key(KeyCode::ArrowLeft, KeyCode::ArrowRight) * KEY_TURN_SPEED * dt;
        self.pitch -= dy * MOUSE_SENSITIVITY;
        self.pitch += key(KeyCode::ArrowUp, KeyCode::ArrowDown) * KEY_TURN_SPEED * dt;
        self.pitch = self.pitch.clamp(-MAX_PITCH, MAX_PITCH);
        // Recoil eases back by itself (frame-rate independent).
        self.recoil *= (-10.0 * dt).exp();

        // Physics moves the body 60 times a second; easing the camera
        // towards it hides the steps on faster displays.
        self.place_camera(ctx, 1.0 - (-40.0 * dt).exp());
    }

    /// Moves the camera `blend` of the way (0 to 1) to the player's eyes and
    /// points it where the player looks.
    pub fn place_camera(&self, ctx: &mut Context, blend: f32) {
        if let Some(position) = self.position(ctx) {
            let eyes = position + Vec3::Y * EYE_HEIGHT;
            ctx.camera.position = ctx.camera.position.lerp(eyes, blend);
        }
        let pitch = (self.pitch + self.recoil).clamp(-MAX_PITCH, MAX_PITCH);
        ctx.camera.set_yaw_pitch(self.yaw, pitch);
    }
}

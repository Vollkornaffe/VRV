use std::{f32::consts::PI, time::Instant};

use cgmath::{vec3, Matrix4, Point3, Quaternion};
use openxr::{Fovf, Posef};
use winit::event::VirtualKeyCode;

#[derive(Copy, Clone, Debug)]
pub struct SphereCoords {
    phi: f32,
    theta: f32,
    radius: f32,
    speed: f32,
    check: Instant,
}

pub enum KeyMap {
    Up,
    Down,
    Left,
    Right,
    Closer,
    Farther,
    UNDEFINED,
}

impl From<VirtualKeyCode> for KeyMap {
    fn from(code: VirtualKeyCode) -> Self {
        match code {
            VirtualKeyCode::W => Self::Up,
            VirtualKeyCode::S => Self::Down,
            VirtualKeyCode::A => Self::Left,
            VirtualKeyCode::D => Self::Right,
            VirtualKeyCode::Q => Self::Closer,
            VirtualKeyCode::E => Self::Farther,
            _ => {
                log::warn!("This key isn't bound");
                Self::UNDEFINED
            }
        }
    }
}

impl SphereCoords {
    pub fn new() -> Self {
        Self {
            phi: std::f32::consts::FRAC_PI_2,
            theta: std::f32::consts::FRAC_PI_4,
            radius: 4.0,
            speed: 2.0,
            check: Instant::now(),
        }
    }

    pub fn update(&mut self, pressed_keys: &[KeyMap]) {
        let d = self.check.elapsed().as_secs_f32() * self.speed;
        self.check = Instant::now();

        for key in pressed_keys {
            match key {
                KeyMap::Up => self.theta -= d,
                KeyMap::Down => self.theta += d,
                KeyMap::Left => self.phi += d,
                KeyMap::Right => self.phi -= d,
                KeyMap::Closer => self.radius -= d,
                KeyMap::Farther => self.radius += d,
                KeyMap::UNDEFINED => {}
            }
        }

        self.theta = self.theta.clamp(0.1, PI - 0.1);
        self.phi %= 2.0 * PI;
        self.radius = self.radius.clamp(0.0, 100.0);
    }

    pub fn to_coords(&self) -> Point3<f32> {
        [
            self.radius * self.phi.cos() * self.theta.sin(),
            self.radius * self.theta.cos(),
            self.radius * self.phi.sin() * self.theta.sin(),
        ]
        .into()
    }
}

pub fn pose_to_matrix_inverse(pose: Posef) -> Matrix4<f32> {
    Matrix4::from(Quaternion::new(
        pose.orientation.w,
        -pose.orientation.x,
        -pose.orientation.y,
        -pose.orientation.z,
    )) * Matrix4::from_translation(vec3(-pose.position.x, -pose.position.y, -pose.position.z))
}

// there are 4 angles to consider instead of one
pub fn fov_to_projection(fov: Fovf) -> Matrix4<f32> {
    let tan_left = fov.angle_left.tan();
    let tan_right = fov.angle_right.tan();
    let tan_down = fov.angle_down.tan();
    let tan_up = fov.angle_up.tan();
    let near = 0.1;
    let far = 100.0;

    let tan_width = tan_right - tan_left;
    let tan_height = tan_down - tan_up;

    Matrix4::new(
        2.0 / tan_width,
        0.0,
        0.0,
        0.0,
        0.0,
        2.0 / tan_height,
        0.0,
        0.0,
        (tan_right + tan_left) / tan_width,
        (tan_up + tan_down) / tan_height,
        -far / (far - near),
        -1.0,
        0.0,
        0.0,
        -(far * near) / (far - near),
        0.0,
    )
}

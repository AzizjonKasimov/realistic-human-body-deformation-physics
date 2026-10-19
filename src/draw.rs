//! How the world looks: the bodies' flesh, bones, and wounds, blood, and the
//! tool in hand, drawn with macroquad as the sandbox app shows them. Anything
//! that shows the world, the sandbox and the game built on this engine, draws
//! it here, so they look alike; each draws its own buttons and labels around
//! it. Drawing only reads the world: what the simulation did, no more and no
//! larger.
//!
//! A frame draws [`draw_background`] in screen pixels, then, under a
//! [`WorldCamera`] for the view the world is shown through,
//! [`draw_body_layers`], [`draw_effects`], and [`draw_tool`] in the world's
//! own pixels.

mod blood;
mod body;
mod bones;
mod pieces;
mod rim;
mod shapes;
mod tools;

use crate as rp;
use macroquad::prelude::*;

use blood::*;
use bones::*;
use pieces::*;
use rim::*;
use shapes::*;

pub use blood::draw_effects;
pub use body::draw_body_layers;
pub use rim::SkinRim;
pub use shapes::{add, bone_point, draw_line_vec, length, mid, normalized, scale, sub, to_mq};
pub use tools::draw_tool;

/// The colours the world and the apps' labels are drawn in.
#[derive(Clone, Copy)]
pub struct Palette {
    pub background: Color,
    pub background_low: Color,
    pub floor: Color,
    pub floor_edge: Color,
    pub skin_base: Color,
    pub skin_light: Color,
    pub skin_mid: Color,
    pub skin_shade: Color,
    pub skin_heat: Color,
    /// Skin reddened by a blow.
    pub skin_flush: Color,
    pub skin_contusion: Color,
    pub skin_outline: Color,
    pub skin_wire: Color,
    pub muscle_base: Color,
    pub muscle_hot: Color,
    pub muscle_contusion: Color,
    pub muscle_shadow: Color,
    /// Flesh crushed or torn away from the sheets, and where it is most
    /// pulped.
    pub crushed_flesh: Color,
    pub crushed_flesh_deep: Color,
    pub bone: Color,
    pub bone_fractured: Color,
    pub bone_shadow: Color,
    pub blood_dark: Color,
    pub blood_mid: Color,
    pub blood_fresh: Color,
    pub blood_stain: Color,
    pub major_vessel: Color,
    pub major_vessel_shadow: Color,
    pub wound_core: Color,
    pub wound_edge: Color,
    pub wound_shadow: Color,
    pub attachment: Color,
    pub hud_back: Color,
    pub hud_border: Color,
    pub hud_text: Color,
    pub hud_muted: Color,
    pub tool_accent: Color,
}

impl Default for Palette {
    fn default() -> Self {
        Palette {
            background: rgba(15, 15, 17, 255),
            background_low: rgba(26, 22, 22, 255),
            floor: rgba(35, 29, 26, 255),
            floor_edge: rgba(92, 64, 55, 255),
            skin_base: rgba(152, 101, 83, 246),
            skin_light: rgba(201, 146, 119, 250),
            skin_mid: rgba(184, 130, 105, 250),
            skin_shade: rgba(92, 54, 44, 255),
            skin_heat: rgba(223, 77, 55, 246),
            skin_flush: rgba(206, 88, 78, 246),
            skin_contusion: rgba(55, 31, 86, 235),
            skin_outline: rgba(69, 40, 36, 220),
            skin_wire: rgba(136, 86, 75, 130),
            muscle_base: rgba(112, 22, 31, 190),
            muscle_hot: rgba(190, 35, 47, 225),
            muscle_contusion: rgba(49, 13, 61, 215),
            muscle_shadow: rgba(47, 8, 13, 150),
            crushed_flesh: rgba(116, 20, 28, 248),
            crushed_flesh_deep: rgba(66, 7, 13, 248),
            bone: rgba(222, 211, 181, 240),
            bone_fractured: rgba(255, 245, 218, 255),
            bone_shadow: rgba(53, 43, 35, 160),
            blood_dark: rgba(43, 2, 7, 235),
            blood_mid: rgba(103, 7, 15, 230),
            blood_fresh: rgba(190, 22, 26, 235),
            blood_stain: rgba(38, 0, 7, 225),
            major_vessel: rgba(168, 12, 24, 230),
            major_vessel_shadow: rgba(28, 0, 5, 210),
            wound_core: rgba(34, 0, 5, 230),
            wound_edge: rgba(156, 18, 24, 235),
            wound_shadow: rgba(17, 0, 4, 225),
            attachment: rgba(70, 148, 235, 48),
            hud_back: rgba(13, 13, 15, 198),
            hud_border: rgba(118, 97, 83, 170),
            hud_text: rgba(232, 226, 212, 245),
            hud_muted: rgba(168, 156, 143, 220),
            tool_accent: rgba(255, 188, 66, 245),
        }
    }
}

/// One frame's picture of the world: what to draw, and the screen it lands
/// on.
pub struct Scene<'a> {
    pub world: &'a rp::World,
    /// The skin's silhouette edge, built from `world`'s mesh
    /// ([`SkinRim::of`], kept up to date with [`SkinRim::refresh`]).
    pub rim: &'a SkinRim,
    pub palette: Palette,
    /// The window's size in screen pixels.
    pub width: f32,
    pub height: f32,
    /// Where the floor the body stands on is drawn on the screen, which the
    /// ground is drawn down from ([`ground_on_screen`]).
    pub floor_y: f32,
    /// The same floor in the world, which blood pools on.
    pub world_floor_y: f32,
    /// See-through skin with the muscle and bones showing, for checking
    /// the anatomy; players only get the normal view.
    pub anatomy: bool,
    /// Also mark where each wound bleeds from and where the tool presses
    /// hardest, a debugging aid.
    pub debug: bool,
    /// Draw the tool in hand, if one is in the world.
    pub show_tool: bool,
    /// How bloody the tool is, from 0 to 1.
    pub tool_blood: f32,
    /// The hand presses the tool: the knife's steel catches the light.
    pub pressing: bool,
}

/// Draws world positions where the app's view puts them on the screen: the
/// screen's own projection, which macroquad's default camera uses, after the
/// view's scale and offset.
pub struct WorldCamera(Mat4);

impl WorldCamera {
    pub fn of(view: rp::View) -> Self {
        let screen = Mat4::orthographic_rh_gl(0.0, screen_width(), screen_height(), 0.0, -1.0, 1.0);
        let scale = view.scale as f32;
        let placed = Mat4::from_translation(vec3(view.offset.x as f32, view.offset.y as f32, 0.0))
            * Mat4::from_scale(vec3(scale, scale, 1.0));
        Self(screen * placed)
    }
}

impl Camera for WorldCamera {
    fn matrix(&self) -> Mat4 {
        self.0
    }

    fn depth_enabled(&self) -> bool {
        false
    }

    fn render_pass(&self) -> Option<RenderPass> {
        None
    }

    fn viewport(&self) -> Option<(i32, i32, i32, i32)> {
        None
    }
}

/// Where the ground begins on the screen: the floor the bodies stand on, at
/// their soles, as `view` draws it, or `bottom` (the window's lowest strip
/// the ground may start at) when that lies higher or there is no body.
pub fn ground_on_screen(world: &rp::World, view: rp::View, bottom: f32) -> f32 {
    let floor = world.floor_y();
    if floor.is_finite() {
        let drawn = view.to_screen(rp::Vec2 { x: 0.0, y: floor }).y;
        (drawn as f32).min(bottom)
    } else {
        bottom
    }
}

pub fn draw_background(ctx: &Scene) {
    clear_background(ctx.palette.background);
    draw_rectangle(
        0.0,
        ctx.height * 0.54,
        ctx.width,
        ctx.height * 0.46,
        ctx.palette.background_low,
    );
    // The ground the body stands on, down to the window's bottom; the
    // control buttons lie on it.
    draw_rectangle(
        0.0,
        ctx.floor_y,
        ctx.width,
        ctx.height - ctx.floor_y,
        ctx.palette.floor,
    );
    draw_line(
        0.0,
        ctx.floor_y,
        ctx.width,
        ctx.floor_y,
        1.0,
        ctx.palette.floor_edge,
    );
    for i in 0..6 {
        let y = ctx.floor_y + 7.0 + i as f32 * 5.0;
        let alpha = 0.04 + i as f32 * 0.012;
        draw_line(
            0.0,
            y,
            ctx.width,
            y,
            1.0,
            with_alpha(rgba(104, 65, 52, 255), alpha),
        );
    }
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

pub fn with_alpha(mut color: Color, alpha: f32) -> Color {
    color.a = alpha.clamp(0.0, 1.0);
    color
}

pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::from_rgba(r, g, b, a)
}

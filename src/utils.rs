use crate::prelude::*;
use posh::{
    gl,
    sl::{branch, Vec2, F32},
    Block, BlockDom, Sl, UniformInterface, UniformInterfaceDom,
};

#[derive(Clone, UniformInterface)]
pub struct Inner<D: UniformInterfaceDom> {
    pub res: D::ColorSampler2d<sl::Vec2>,
}

#[derive(Clone, Copy, Block)]
#[repr(C)]
pub struct App<D: BlockDom> {
    pub size: D::UVec2,
}

pub const fn fragcoord(clip_space_pos: Vec2, window_size: Vec2) -> Vec2 {
    uv(clip_space_pos) * window_size
}

pub const fn uv(clip_space_pos: Vec2) -> Vec2 {
    clip_space_pos * 0.5 + 0.5
}

pub const fn flip_v(uv: Vec2) -> Vec2 {
    Vec2 {
        x: uv.x,
        y: 1.0 - uv.y,
    }
}

pub const fn full_screen_quad() -> [gl::Vec2; 6] {
    [
        gl::Vec2 { x: -1.0, y: 1.0 },
        gl::Vec2 { x: -1.0, y: -1.0 },
        gl::Vec2 { x: 1.0, y: -1.0 },
        gl::Vec2 { x: 1.0, y: -1.0 },
        gl::Vec2 { x: 1.0, y: 1.0 },
        gl::Vec2 { x: -1.0, y: 1.0 },
    ]
}

pub fn texture_aspect_ratio<C: ColorSample>(sampler: sl::ColorSampler2d<C>) -> F32 {
    let size: Vec2 = sampler.size(0u32).as_vec2();
    aspect_ratio(size)
}

pub fn aspect_ratio(size: Vec2) -> F32 {
    size.x / size.y
}

pub fn preserve_aspect_ratio(viewport_aspect: F32, texture_aspect: F32, uv: Vec2) -> Vec2 {
    branch(
        viewport_aspect.ge(texture_aspect),
        Vec2::new(uv.x * (viewport_aspect / texture_aspect), uv.y),
        Vec2::new(uv.x, uv.y * (texture_aspect / viewport_aspect)),
    )
}

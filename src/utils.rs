use crate::prelude::*;
use posh::{gl, Block, BlockDom, Sl, UniformInterface, UniformInterfaceDom};

#[derive(Clone, UniformInterface)]
pub struct Inner<D: UniformInterfaceDom> {
    pub res: D::ColorSampler2d<sl::Vec2>,
}

#[derive(Clone, Copy, Block)]
#[repr(C)]
pub struct App<D: BlockDom> {
    pub res: D::Vec2,
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

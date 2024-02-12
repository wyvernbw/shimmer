use std::error::Error;

use posh::{sl, Block, BlockDom, Sl};
use shimmer::{Program, RunMode, WindowConfig};
use winit::dpi::PhysicalSize;

pub fn main() -> Result<(), Box<dyn Error + 'static>> {
    let program: Program<Uniforms<Sl>, sl::Vec2> = Program::new(
        vertex_shader,
        fragment_shader,
        RunMode::Windowed(Some(WindowConfig {
            title: "My window".into(),
            size: PhysicalSize::new(800, 600),
            draw_mode: shimmer::DrawMode::Loop { framerate: 144 },
        })),
    )?;
    // method not found in `Program<Uniforms<Sl>, Vec2>`
    //
    // `serve` only exists on `Program` after `with_vertices`, `with_uniforms` (optionally), and `with_draw_settings` have been called
    program.serve()?;
    Ok(())
}

/// Define shader uniforms
#[derive(Debug, Clone, Copy, Block)]
#[repr(C)]
struct Uniforms<D: BlockDom> {
    foo: D::F32,
}

fn vertex_shader(globals: Uniforms<Sl>, vertex: sl::Vec2) -> sl::VsOutput<sl::Vec2> {
    sl::VsOutput {
        clip_position: sl::vec4(vertex.x, vertex.y, 0.0, 1.0),
        interpolant: vertex,
    }
}

fn fragment_shader(globals: Uniforms<Sl>, vertex: sl::Vec2) -> sl::Vec4 {
    sl::vec4(vertex.x, vertex.y, 0.5, 1.0)
}

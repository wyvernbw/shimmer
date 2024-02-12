use std::time::Instant;

use posh::{
    gl::{self, BufferUsage, DrawSettings, PrimitiveMode, UniformBuffer},
    sl, Block, BlockDom, Gl, Sl,
};
use shimmer::{Program, RunMode, WindowConfig};
use winit::dpi::PhysicalSize;

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let program: Program<Uniforms<Sl>, sl::Vec2> = Program::new(
        vertex_shader,
        fragment_shader,
        RunMode::Windowed(Some(WindowConfig {
            title: "My window".into(),
            size: PhysicalSize::new(800, 600),
            draw_mode: shimmer::DrawMode::Loop { framerate: 60 },
        })),
    )?;
    let start = Instant::now();
    let program = program
        .with_vertices(move |gl| {
            let vertices: gl::VertexBuffer<gl::Vec2> = gl
                .create_vertex_buffer(
                    &[
                        [0.0f32, 1.0].into(),
                        [-0.5, -0.5].into(),
                        [0.5, -0.5].into(),
                    ],
                    gl::BufferUsage::StreamDraw,
                )
                .unwrap();
            vertices.as_vertex_spec(PrimitiveMode::Triangles)
        })
        .with_uniforms(move |gl: &gl::Context| {
            let uniforms = Uniforms {
                time: Instant::now().duration_since(start).as_secs_f32(),
                size: 1.0,
            };
            let uniforms: UniformBuffer<Uniforms<Gl>> = gl
                .create_uniform_buffer(uniforms, BufferUsage::StaticRead)
                .unwrap();
            uniforms.as_binding()
        })
        .with_draw_settings(|gl: &gl::Context| DrawSettings {
            clear_color: Some([1.0, 1.0, 1.0, 1.0]),
            ..Default::default()
        });
    program.serve()?;
    Ok(())
}

/// Define shader uniforms
#[derive(Debug, Clone, Copy, Block)]
#[repr(C)]
struct Uniforms<D: BlockDom> {
    time: D::F32,
    size: D::F32,
}

fn vertex_shader(globals: Uniforms<Sl>, vertex: sl::Vec2) -> sl::VsOutput<sl::Vec2> {
    let position = sl::Vec2::from_angle(globals.time).rotate(vertex * globals.size);

    sl::VsOutput {
        clip_position: sl::vec4(position.x, position.y, 0.0, 1.0),
        interpolant: vertex,
    }
}

fn fragment_shader(globals: Uniforms<Sl>, interpolant: sl::Vec2) -> sl::Vec4 {
    let rg = (interpolant + globals.time).cos().powf(2.0);

    sl::vec4(rg.x, rg.y, 0.5, 1.0)
}

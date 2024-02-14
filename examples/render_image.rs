use posh::gl::Sampler2dSettings;
use shimmer::{
    prelude::*,
    utils::{full_screen_quad, uv},
};

#[derive(UniformInterface)]
struct Uniforms<D: UniformInterfaceDom> {
    texture: D::ColorSampler2d<sl::Vec4>,
    app: D::Block<App<Sl>>,
}

#[allow(clippy::unwrap_used)]
fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let image = image::open("examples/assets/Dog.png")?;
    let program: Program<Uniforms<Sl>, sl::Vec2> = Program::new(
        vertex_shader,
        fragment_shader,
        RunMode::Windowed(Some(WindowConfig {
            title: "My window".into(),
            draw_mode: shimmer::DrawMode::Loop { framerate: 144.0 },
            ..Default::default()
        })),
    )?;

    let program = program
        .with_vertices(|handle: Handle| {
            handle
                .create_vertex_spec(
                    &full_screen_quad(),
                    BufferUsage::StreamDraw,
                    PrimitiveMode::Triangles,
                )
                .unwrap()
        })
        .with_uniforms(move |handle: Handle| {
            let image = image.clone();
            let image = gl::ColorImage::rgba_u8_slice(
                [image.width(), image.height()],
                image.as_rgba8().unwrap().as_raw(),
            );
            Uniforms {
                texture: handle
                    .gl()
                    .create_color_texture_2d(image)
                    .unwrap()
                    .as_color_sampler(Sampler2dSettings::default()),
                app: handle.app_buffer().unwrap(),
            }
        })
        .with_draw_settings(|_| DrawSettings {
            clear_color: Some([1.0, 1.0, 1.0, 1.0]),
            ..Default::default()
        });
    program.serve()?;
    Ok(())
}

fn vertex_shader(_: Uniforms<Sl>, vertex: sl::Vec2) -> sl::VsOutput<sl::Vec2> {
    sl::VsOutput {
        clip_position: sl::vec4(vertex.x, vertex.y, 0.0, 1.0),
        interpolant: vertex,
    }
}

fn fragment_shader(Uniforms { texture, app }: Uniforms<Sl>, clip_space_pos: sl::Vec2) -> sl::Vec4 {
    // Calculate and flip the UV coordinate from the clip space position
    let uv = flip_v(uv(clip_space_pos));
    // Preserve the aspect ratio of the texture
    let uv = preserve_aspect_ratio(
        aspect_ratio(app.size.as_vec2()),
        texture_aspect_ratio(texture),
        uv,
    );
    // Sample the texture and lerp the color based on the UV coordinate
    let color = texture.sample(uv);
    // Create black bars when uv is not in the range `[0, 1]`
    let step = uv.step(1.0) + (uv * -1.0).step(0.0);

    color.lerp(sl::Vec4::new(0.0, 0.0, 0.0, 1.0), step.x + step.y)
}

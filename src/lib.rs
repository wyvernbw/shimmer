use std::error::Error;

use glutin::{
    config::{Api, Config, ConfigTemplateBuilder, GlConfig},
    context::{
        ContextApi, ContextAttributesBuilder, GlContext, NotCurrentGlContext,
        PossiblyCurrentContext, PossiblyCurrentGlContext, Version,
    },
    display::{GetGlDisplay, GlDisplay},
    surface::{Surface, SurfaceTypeTrait, WindowSurface},
};
use glutin_winit::{DisplayBuilder, GlWindow};
use posh::{
    gl,
    sl::{FsFunc, FsSig, VsFunc, VsSig},
    UniformUnion,
};
use raw_window_handle::HasRawWindowHandle;
use winit::{
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

// TODO: Delete this
type QuickError = Box<dyn Error + 'static>;

struct ProgramState {
    config: Config,
    gl: gl::Context,
    event_loop: EventLoop<()>,
    gl_surface: Surface<WindowSurface>,
    window_builder: WindowBuilder,
    window: Window,
}

impl ProgramState {
    // FIXME: Improve error type
    fn new(headless: bool) -> Result<Self, Box<dyn Error + 'static>> {
        let event_loop = EventLoop::new()?;
        let window_builder = WindowBuilder::new()
            .with_title("Posh")
            .with_visible(!headless)
            .with_transparent(true);
        let template = ConfigTemplateBuilder::new().with_api(Api::OPENGL);
        let display = DisplayBuilder::new().with_window_builder(Some(window_builder.clone()));
        let (Some(window), config) = display.build(&event_loop, template, |configs| {
            configs
                .into_iter()
                .find(|config| config.api() == Api::OPENGL)
                .expect("No OpenGL config found")
        })?
        else {
            // FIXME: return better error
            return Err("No OpenGL config found".into());
        };
        tracing::info!("Window {:?} created with config {:?}", window, config);
        let raw_window_handle = window.raw_window_handle();
        let context_attributes = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::OpenGl(Some(Version::new(4, 1))))
            .build(Some(raw_window_handle));
        let display = config.display();
        let version = display.version_string();
        tracing::info!("OpenGL version: {:?}", version);
        let ctx = unsafe { display.create_context(&config, &context_attributes)? };
        tracing::info!("OpenGL context created: {:?}", ctx.context_api());
        let surface_attributes = window.build_surface_attributes(Default::default());
        let gl_surface = unsafe {
            config
                .display()
                .create_window_surface(&config, &surface_attributes)?
        };
        let ctx = ctx.make_current(&gl_surface)?;
        tracing::info!("Context made current: {:?}", ctx.is_current());
        ctx.make_current(&gl_surface)?;
        let features = display.supported_features();
        tracing::info!("Display features {:?}", features);
        let gl = unsafe {
            glow::Context::from_loader_function_cstr(|symbol| display.get_proc_address(symbol))
        };
        let gl = gl::Context::new(gl)?;
        Ok(Self {
            config,
            gl,
            event_loop,
            gl_surface,
            window_builder,
            window,
        })
    }
}

pub struct Program<U, VertexSig, FragSig>
where
    VertexSig: VsSig,
    FragSig: FsSig,
    U: UniformUnion<VertexSig::U, FragSig::U>,
{
    state: ProgramState,
    inner: gl::Program<U, <VertexSig as VsSig>::V, <FragSig as FsSig>::F>,
}

// FIXME: remove unwrap
impl<U, VertexSig, FragSig> Program<U, VertexSig, FragSig>
where
    VertexSig: VsSig<C = ()>,
    FragSig: FsSig<C = (), W = VertexSig::W>,
    U: UniformUnion<VertexSig::U, FragSig::U>,
{
    pub fn new<VertexFn, FragFn>(
        headless: bool,
        vertex_shader: VertexFn,
        fragment_shader: FragFn,
    ) -> Result<Self, QuickError>
    where
        VertexFn: VsFunc<VertexSig>,
        FragFn: FsFunc<FragSig>,
    {
        let state = ProgramState::new(headless).unwrap();
        let inner = state.gl.create_program(vertex_shader, fragment_shader)?;
        Ok(Program { state, inner })
    }

    pub fn headless<VertexFn, FragFn>(
        vertex_shader: VertexFn,
        fragment_shader: FragFn,
    ) -> Result<Self, QuickError>
    where
        VertexFn: VsFunc<VertexSig>,
        FragFn: FsFunc<FragSig>,
    {
        Self::new(true, vertex_shader, fragment_shader)
    }

    pub fn windowed<VertexFn, FragFn>(
        vertex_shader: VertexFn,
        fragment_shader: FragFn,
    ) -> Result<Self, QuickError>
    where
        VertexFn: VsFunc<VertexSig>,
        FragFn: FsFunc<FragSig>,
    {
        Self::new(false, vertex_shader, fragment_shader)
    }
}

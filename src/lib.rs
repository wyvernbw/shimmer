#![feature(effects)]
#![feature(const_trait_impl)]
#![feature(type_changing_struct_update)]
#![feature(associated_type_defaults)]
#![feature(trait_alias)]
use std::{
    error::Error,
    marker::PhantomData,
    process::exit,
    time::{Duration, Instant},
};

use error::{log_error, ErrorKind};
use gl::Context;
use glutin::{
    config::{Api, Config, ConfigTemplateBuilder, GlConfig},
    context::{
        ContextApi, ContextAttributesBuilder, GlContext, GlProfile, NotCurrentGlContext,
        PossiblyCurrentContext, PossiblyCurrentGlContext, Version,
    },
    display::{GetGlDisplay, GlDisplay},
    surface::{GlSurface, Surface, WindowSurface},
};
use glutin_winit::{DisplayBuilder, GlWindow};
use posh::{
    bytemuck::Pod,
    gl::{BufferError, BufferUsage, UniformBuffer, UniformBufferBinding},
    *,
};
use posh::{
    gl::VertexSpec,
    sl::{ColorSample, FsFunc, FsSig, VsFunc, VsSig},
};
use posh::{
    gl::{self, PrimitiveMode},
    sl,
};
use prelude::utils::App;
use raw_window_handle::HasRawWindowHandle;
use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::pump_events::EventLoopExtPumpEvents,
    window::{Window, WindowBuilder},
};

pub mod error;
pub mod prelude;
pub mod utils;

struct ProgramState {
    config: Config,
    gl: gl::Context,
    event_loop: EventLoop<()>,
    gl_surface: Surface<WindowSurface>,
    window_builder: WindowBuilder,
    window: Window,
    ctx: PossiblyCurrentContext,
}

impl ProgramState {
    // FIXME: Improve error type
    fn new(run_mode: RunMode) -> Result<Self, ErrorKind> {
        let event_loop = EventLoop::new()?;
        let window_builder = WindowBuilder::new()
            .with_title("Posh")
            .with_visible(!matches!(run_mode, RunMode::Headless))
            .with_transparent(true);

        let window_builder =
            if let RunMode::Windowed(Some(WindowConfig { title, size, .. })) = run_mode {
                window_builder.with_inner_size(size)
            } else {
                window_builder
            };

        let template = ConfigTemplateBuilder::new().with_api(Api::OPENGL);
        let display = DisplayBuilder::new().with_window_builder(Some(window_builder.clone()));
        let (Some(window), config) = display
            .build(&event_loop, template, |configs| {
                let configs = configs.collect::<Vec<_>>();
                let first = configs.first().cloned();
                configs
                    .into_iter()
                    .find(|config| config.api() == Api::OPENGL)
                    .or(first)
                    .expect("No OpenGL config found")
            })
            .map_err(|_| ErrorKind::DisplayError)?
        else {
            // DONE: return better error
            return Err(ErrorKind::WindowError);
        };
        tracing::info!("Window {:?} created with config {:?}", window, config);
        let raw_window_handle = window.raw_window_handle();
        let context_attributes = ContextAttributesBuilder::new()
            .with_profile(GlProfile::Core)
            .with_context_api(ContextApi::OpenGl(Some(Version::new(4, 1))))
            .build(Some(raw_window_handle));
        let display = config.display();
        let version = display.version_string();
        tracing::info!("OpenGL version: {:?}", version);
        let ctx = unsafe {
            display
                .create_context(&config, &context_attributes)
                .map_err(|_| ErrorKind::OpenGlError("Context creation failed".into()))?
        };
        tracing::info!("OpenGL context created: {:?}", ctx.context_api());
        let surface_attributes = window.build_surface_attributes(Default::default());
        let gl_surface = unsafe {
            config
                .display()
                .create_window_surface(&config, &surface_attributes)
                .map_err(|_| ErrorKind::OpenGlError("Failed to create gl surface".into()))?
        };
        let ctx = ctx
            .make_current(&gl_surface)
            .map_err(|_| ErrorKind::OpenGlError("Failed to make context current".into()))?;
        tracing::info!("Context made current: {:?}", ctx.is_current());
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
            ctx,
        })
    }
}

pub struct WithVertices;
pub struct WithUniforms;
pub struct WithDrawSettings;
pub struct WithoutVertices;
pub struct WithoutUniforms;
pub struct WithoutDrawSettings;

pub struct Program<
    U,
    V,
    F = sl::Vec4,
    HasVertices = WithoutVertices,
    HasUniforms = WithoutUniforms,
    HasSettings = WithoutDrawSettings,
> where
    U: UniformInterface<Sl>,
    V: VsInterface<Sl>,
    F: ColorSample,
{
    state: ProgramState,
    run_mode: RunMode,
    inner: gl::Program<U, V, F>,
    workflow: Workflow<U, V, F>,
    _marker: PhantomData<(HasVertices, HasUniforms, HasSettings)>,
}

pub struct Handle<'a>(&'a ProgramState);
pub trait VertexFn<V: VsInterface<Sl>> = Fn(Handle) -> VertexSpec<V>;
pub trait UniformsFn<U: UniformInterface<Sl>> = Fn(Handle) -> <U as UniformInterface<Sl>>::Gl;
pub trait SettingsFn = Fn(Handle) -> gl::DrawSettings;

type VertexCallback<V> = Box<dyn VertexFn<V>>;
type UniformsCallback<U> = Box<dyn UniformsFn<U>>;
type SettingsCallback = Box<dyn SettingsFn>;

pub struct Workflow<U, V, F>
where
    U: UniformInterface<Sl>,
    V: VsInterface<Sl>,
    F: ColorSample,
{
    vertex_spec: Option<VertexCallback<V>>,
    uniforms: Option<UniformsCallback<U>>,
    settings: Option<SettingsCallback>,
    _marker: PhantomData<F>,
}

impl<U: UniformInterface<Sl> + 'static, V: VsInterface<Sl> + 'static, F: ColorSample>
    Program<U, V, F>
{
    pub fn new<FFn, VFn, FSig, VSig>(
        vertex_shader: VFn,
        fragment_shader: FFn,
        run_mode: RunMode,
    ) -> Result<Self, ErrorKind>
    where
        VSig: VsSig<C = (), V = V>,
        FSig: FsSig<C = (), W = VSig::W, F = F>,
        VFn: VsFunc<VSig>,
        FFn: FsFunc<FSig>,
        U: UniformUnion<VSig::U, FSig::U>,
    {
        let state = ProgramState::new(run_mode.clone())?;
        let inner: gl::Program<U, V, F> =
            state.gl.create_program(vertex_shader, fragment_shader)?;
        Ok(Program {
            state,
            run_mode,
            inner,
            _marker: PhantomData,
            workflow: Workflow {
                vertex_spec: None,
                uniforms: None,
                settings: None,
                _marker: PhantomData,
            },
        })
    }

    pub fn with_vertices(
        self,
        vertices: impl VertexFn<V> + 'static,
    ) -> Program<U, V, F, WithVertices, WithoutUniforms, WithoutDrawSettings> {
        Program {
            workflow: Workflow {
                vertex_spec: Some(Box::new(vertices)),
                ..self.workflow
            },
            state: self.state,
            run_mode: self.run_mode,
            inner: self.inner,
            _marker: PhantomData,
        }
    }

    pub fn with_draw_settings(
        self,
        settings: impl SettingsFn + 'static,
    ) -> Program<U, V, F, WithoutVertices, WithUniforms, WithoutDrawSettings> {
        Program {
            workflow: Workflow {
                settings: Some(Box::new(settings)),
                ..self.workflow
            },
            state: self.state,
            run_mode: self.run_mode,
            inner: self.inner,
            _marker: PhantomData,
        }
    }
}

impl<U: UniformInterface<Sl> + 'static, V: VsInterface<Sl> + 'static, F: ColorSample, US, DS>
    Program<U, V, F, WithVertices, US, DS>
{
    pub fn with_uniforms(
        self,
        uniforms: impl UniformsFn<U> + 'static,
    ) -> Program<U, V, F, WithVertices, WithUniforms, DS> {
        Program {
            workflow: Workflow {
                uniforms: Some(Box::new(uniforms)),
                ..self.workflow
            },
            state: self.state,
            run_mode: self.run_mode,
            inner: self.inner,
            _marker: PhantomData,
        }
    }
}

impl<U: UniformInterface<Sl> + 'static, V: VsInterface<Sl> + 'static, F: ColorSample, DS>
    Program<U, V, F, WithVertices, WithUniforms, DS>
{
    pub fn with_draw_settings(
        self,
        settings: impl SettingsFn + 'static,
    ) -> Program<U, V, F, WithVertices, WithUniforms, WithDrawSettings> {
        Program {
            workflow: Workflow {
                settings: Some(Box::new(settings)),
                ..self.workflow
            },
            state: self.state,
            run_mode: self.run_mode,
            inner: self.inner,
            _marker: PhantomData,
        }
    }
}

impl<U: UniformInterface<Sl> + 'static, V: VsInterface<Sl> + 'static>
    Program<U, V, sl::Vec4, WithVertices, WithUniforms, WithDrawSettings>
{
    pub fn draw(&self) -> Result<(), ErrorKind> {
        match (
            &self.workflow.vertex_spec,
            &self.workflow.uniforms,
            &self.workflow.settings,
        ) {
            (Some(vertex), Some(uniforms), Some(settings)) => {
                self.inner
                    .with_settings(settings(Handle(&self.state)))
                    .with_uniforms(uniforms(Handle(&self.state)))
                    .draw(vertex(Handle(&self.state)))?;
            }
            _ => {
                unreachable!("Vertex spec, uniforms and settings must be provided")
            }
        }
        Ok(())
    }

    pub fn serve(mut self) -> Result<(), ErrorKind> {
        match self.run_mode {
            RunMode::Headless => todo!("TODO: Implement headless mode"),
            RunMode::Windowed(ref window_config) => {
                let window_config = window_config.clone();
                self.window_loop(window_config)
            }
        }
    }

    fn window_loop(mut self, window_config: Option<WindowConfig>) -> Result<(), ErrorKind> {
        self.draw()?;
        log_error(self.state.gl_surface.swap_buffers(&self.state.ctx));
        loop {
            if let Some(WindowConfig {
                draw_mode: DrawMode::Loop { framerate },
                ..
            }) = window_config
            {
                let time = Instant::now();
                let frame_time = Duration::from_secs_f32(1.0 / framerate as f32);
                self.draw()?;
                self.state.window.request_redraw();
                log_error(self.state.gl_surface.swap_buffers(&self.state.ctx));
                let delta = time.elapsed();
                if delta < frame_time {
                    std::thread::sleep(frame_time - delta);
                }
                #[cfg(feature = "tracing")]
                let _ = log_frame_time(time.elapsed());
            }
            self.state
                .event_loop
                .pump_events(Some(Duration::ZERO), move |event, target| {
                    target.set_control_flow(ControlFlow::Poll);
                    match event {
                        Event::WindowEvent { event, .. } => match event {
                            WindowEvent::RedrawRequested => {}
                            WindowEvent::CloseRequested => {
                                exit(0);
                            }
                            WindowEvent::Resized(_) => {
                                target.set_control_flow(ControlFlow::Poll);
                            }
                            _ => {}
                        },
                        _ => {}
                    }
                });
        }
    }
}

impl<V: VsInterface<Sl> + 'static>
    Program<(), V, sl::Vec4, WithVertices, WithoutUniforms, WithDrawSettings>
{
    pub fn draw(&mut self) -> Result<(), ErrorKind> {
        match (
            &self.workflow.vertex_spec,
            &self.workflow.uniforms,
            &self.workflow.settings,
        ) {
            (Some(vertex), None, Some(settings)) => {
                self.inner
                    .with_settings(settings(Handle(&self.state)))
                    .draw(vertex(Handle(&self.state)))?;
            }
            _ => {
                unreachable!("Vertex spec and settings must be provided")
            }
        }
        Ok(())
    }
}

#[cfg(feature = "tracing")]
fn log_frame_time(time: Duration) -> Result<(), Box<dyn Error + 'static>> {
    use std::io::stdout;

    use crossterm::{
        cursor::MoveUp,
        execute,
        terminal::{Clear, ClearType},
    };

    let ms = time.as_millis();
    execute!(stdout(), Clear(ClearType::CurrentLine))?;
    tracing::info!(name: "frame_time", "Frame time: {:.2}ms\t FPS: {:.2}", ms, 1.0 / time.as_secs_f64());
    execute!(stdout(), MoveUp(1))?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub title: String,
    pub size: PhysicalSize<u32>,
    pub draw_mode: DrawMode,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "Shimmer".into(),
            size: PhysicalSize::new(800, 600),
            draw_mode: DrawMode::Once,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub enum DrawMode {
    #[default]
    Once,
    Loop {
        framerate: u64,
    },
}

#[derive(Debug, Clone)]
pub enum RunMode {
    Headless,
    Windowed(Option<WindowConfig>),
}

impl<'a> Handle<'a> {
    pub fn gl(&self) -> &gl::Context {
        &self.0.gl
    }
    pub fn app(&self) -> App<Gl> {
        let size = self.0.window.inner_size();
        App {
            size: gl::UVec2 {
                x: size.width,
                y: size.height,
            },
        }
    }
    pub fn app_buffer(&self) -> Result<UniformBufferBinding<App<Sl>>, BufferError> {
        Ok(self
            .gl()
            .create_uniform_buffer::<App<Gl>>(self.app(), BufferUsage::DynamicDraw)?
            .as_binding())
    }
    pub fn create_uniform_binding<B: Block<Gl>>(
        &self,
        data: B::Gl,
        usage: BufferUsage,
    ) -> Result<UniformBufferBinding<B::Sl>, BufferError> {
        Ok(self
            .gl()
            .create_uniform_buffer::<B>(data, usage)?
            .as_binding())
    }
    pub fn create_vertex_spec<V: Block<Gl> + Pod>(
        &self,
        vertices: &[V],
        usage: BufferUsage,
        primitive_mode: PrimitiveMode,
    ) -> Result<gl::VertexSpec<<V as Block<Gl>>::Sl>, ErrorKind> {
        Ok(self
            .0
            .gl
            .create_vertex_buffer(vertices, usage)?
            .as_vertex_spec(primitive_mode))
    }
}

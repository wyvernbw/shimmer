#![feature(type_changing_struct_update)]
#![feature(associated_type_defaults)]
#![feature(trait_alias)]
use std::{
    error::Error,
    marker::PhantomData,
    process::exit,
    time::{Duration, Instant},
};

use glutin::{
    config::{Api, Config, ConfigTemplateBuilder, GlConfig},
    context::{
        ContextApi, ContextAttributesBuilder, GlContext, NotCurrentGlContext,
        PossiblyCurrentContext, PossiblyCurrentGlContext, Version,
    },
    display::{GetGlDisplay, GlDisplay},
    surface::{GlSurface, Surface, WindowSurface},
};
use glutin_winit::{DisplayBuilder, GlWindow};
use posh::{
    gl::VertexSpec,
    sl::{ColorSample, FsFunc, FsSig, VsFunc, VsSig},
    Sl, UniformInterface, UniformUnion, VsInterface,
};
use raw_window_handle::HasRawWindowHandle;
use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    platform::pump_events::EventLoopExtPumpEvents,
    raw_window_handle::HasDisplayHandle,
    window::{Window, WindowBuilder},
};

pub use gl::Context;
pub use posh::gl;
pub use posh::sl;

// TODO: Delete this
type QuickError = Box<dyn Error + 'static>;

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
    fn new(run_mode: RunMode) -> Result<Self, Box<dyn Error + 'static>> {
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
            ctx,
        })
    }
}

pub struct Program<U, V, F = sl::Vec4>
where
    U: UniformInterface<Sl>,
    V: VsInterface<Sl>,
    F: ColorSample,
{
    state: ProgramState,
    run_mode: RunMode,
    inner: gl::Program<U, V, F>,
    workflow: Workflow<U, V, F>,
}

pub trait VertexFn<V: VsInterface<Sl>> = Fn(&gl::Context) -> VertexSpec<V>;
pub trait UniformsFn<U: UniformInterface<Sl>> = Fn(&gl::Context) -> <U as UniformInterface<Sl>>::Gl;
pub trait SettingsFn = Fn(&gl::Context) -> gl::DrawSettings;

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
    ) -> Result<Self, QuickError>
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
            workflow: Workflow {
                vertex_spec: None,
                uniforms: None,
                settings: None,
                _marker: PhantomData,
            },
        })
    }

    pub fn with_vertices(self, vertices: impl VertexFn<V> + 'static) -> Self {
        Self {
            workflow: Workflow {
                vertex_spec: Some(Box::new(vertices)),
                ..self.workflow
            },
            ..self
        }
    }

    pub fn with_uniforms(self, uniforms: impl UniformsFn<U> + 'static) -> Self {
        Self {
            workflow: Workflow {
                uniforms: Some(Box::new(uniforms)),
                ..self.workflow
            },
            ..self
        }
    }

    pub fn with_draw_settings(self, settings: impl SettingsFn + 'static) -> Self {
        Self {
            workflow: Workflow {
                settings: Some(Box::new(settings)),
                ..self.workflow
            },
            ..self
        }
    }
}

impl<U: UniformInterface<Sl> + 'static, V: VsInterface<Sl> + 'static> Program<U, V, sl::Vec4> {
    pub fn draw(&self) -> Result<(), QuickError> {
        match (
            &self.workflow.vertex_spec,
            &self.workflow.uniforms,
            &self.workflow.settings,
        ) {
            (None, _, _) => {}
            (Some(_), None, _) => {}
            (Some(vertex), Some(uniforms), None) => {
                self.inner
                    .with_uniforms(uniforms(&self.state.gl))
                    .draw(vertex(&self.state.gl))?;
            }
            (Some(vertex), Some(uniforms), Some(settings)) => {
                self.inner
                    .with_settings(settings(&self.state.gl))
                    .with_uniforms(uniforms(&self.state.gl))
                    .draw(vertex(&self.state.gl))?;
            }
        }
        Ok(())
    }

    pub fn serve(mut self) -> Result<(), QuickError> {
        match self.run_mode {
            RunMode::Headless => todo!(),
            RunMode::Windowed(ref window_config) => {
                self.draw()?;
                self.state.gl_surface.swap_buffers(&self.state.ctx)?;
                loop {
                    let (tx, rx) = std::sync::mpsc::channel::<()>();
                    tx.send(())?;
                    let timeout = if let Some(WindowConfig {
                        draw_mode: DrawMode::Loop { framerate },
                        ..
                    }) = window_config
                    {
                        Some(Duration::from_secs(1) / *framerate as u32)
                    } else {
                        None
                    };
                    self.state
                        .event_loop
                        .pump_events(timeout, move |event, target| match event {
                            Event::WindowEvent { event, .. } => match event {
                                WindowEvent::RedrawRequested => {
                                    tx.send(()).unwrap();
                                }
                                WindowEvent::CloseRequested => {
                                    exit(0);
                                }
                                _ => {}
                            },
                            Event::Suspended => {}
                            _ => {}
                        });
                    if let Some(WindowConfig {
                        draw_mode: DrawMode::Loop { framerate },
                        title,
                        size,
                    }) = window_config
                    {
                        let time = Instant::now();
                        let frame_time = Duration::from_secs_f32(1.0 / *framerate as f32);
                        if let Ok(()) = rx.recv() {
                            self.draw()?;
                            self.state.gl_surface.swap_buffers(&self.state.ctx)?;
                        }
                        let delta = time.elapsed();
                        if delta < frame_time {
                            std::thread::sleep(frame_time - delta);
                        }
                        #[cfg(feature = "tracing")]
                        log_frame_time(time.elapsed())?;
                        self.state.window.request_redraw();
                        self.state.window.set_title(title);
                    }
                }
            }
        }
    }
}

impl<V: VsInterface<Sl> + 'static> Program<(), V, sl::Vec4> {
    pub fn draw_no_uniforms(&mut self) -> Result<(), QuickError> {
        match &self.workflow {
            Workflow {
                vertex_spec: Some(vertex_spec),
                uniforms: Some(uniforms),
                ..
            } => {
                unreachable!();
            }
            Workflow {
                vertex_spec: Some(vertex_spec),
                uniforms: None,
                ..
            } => {
                self.inner.draw(vertex_spec(&self.state.gl))?;
            }
            Workflow {
                vertex_spec: None, ..
            } => {
                return Err("No vertex buffer provided!".into());
            }
        }
        Ok(())
    }
}

#[cfg(feature = "tracing")]
fn log_frame_time(time: Duration) -> Result<(), Box<dyn Error + 'static>> {
    use std::io::stdout;

    use crossterm::{
        cursor::{self, MoveDown, MoveUp},
        execute,
        terminal::{Clear, ClearType},
    };

    let position = cursor::position()?;
    let ms = time.as_millis();
    execute!(stdout(), Clear(ClearType::CurrentLine))?;
    tracing::info!(name: "frame_time", "Frame time: {:.2}ms\t FPS: {:.2}", ms, 1.0 / time.as_secs_f64());
    execute!(stdout(), MoveUp(1))?;
    Ok(())
}

#[derive(Debug, Default, Clone)]
pub struct WindowConfig {
    pub title: String,
    pub size: PhysicalSize<u32>,
    pub draw_mode: DrawMode,
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

pub fn full_screen_quad() -> Vec<gl::Vec2> {
    vec![
        [-1.0, 1.0].into(),
        [-1.0, -1.0].into(),
        [1.0, -1.0].into(),
        [1.0, -1.0].into(),
        [1.0, 1.0].into(),
        [-1.0, 1.0].into(),
    ]
}

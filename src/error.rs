use std::error::Error;

use posh::gl::{BufferError, ContextError, DrawError, ProgramError};
use winit::error::EventLoopError;

#[derive(Debug, thiserror::Error)]
pub enum ErrorKind {
    #[error("Display could not be created")]
    DisplayError,
    #[error("No OpenGL Config found for the current platform.")]
    NoConfigFound,
    #[error("EventLoopError: {0}")]
    EventLoopError(#[from] EventLoopError),
    #[error("Error creating window")]
    WindowError,
    #[error("OpenGl Error")]
    OpenGlError(String),
    #[error("PoshContextError: {0}")]
    PoshContextError(#[from] ContextError),
    #[error("PoshProgramError: {0}")]
    PoshProgramError(#[from] ProgramError),
    #[error("PoshDrawError: {0}")]
    PoshDrawError(#[from] DrawError),
    #[error("PoshBufferError: {0}")]
    PoshBufferError(#[from] BufferError),
}

pub(crate) fn log_error<T>(res: Result<T, impl Error>) {
    if let Err(e) = res {
        tracing::error!("{}", e);
    }
}

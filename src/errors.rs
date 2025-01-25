use thiserror::Error;

pub type Result<T> = std::result::Result<T, DebuggerError>;

#[derive(Error, Debug)]
pub enum DebuggerError {
    #[error("Os error: {0}")]
    Os(#[from] nix::Error),
    #[error("Io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Executable does not exist: {0}")]
    ExecutableDoesNotExist(String),
    #[error("Executable is not a file: {0}")]
    ExecutableIsNotAFile(String),
    #[error("Could not convert to CString: {0}")]
    CStringConv(#[from] std::ffi::NulError),
    #[error("No debuggee configured")]
    NoDebugee,
    #[error("Tried to enable breakpoint again")]
    BreakpointIsAlreadyEnabled,
    #[error("Tried to disable breakpoint again")]
    BreakpointIsAlreadyDisabled,
}

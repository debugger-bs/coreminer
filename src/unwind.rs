//! # Stack Unwinding Module
//!
//! Provides functionality for generating backtraces of the debuggee's call stack.
//!
//! This module utilizes the [mod@unwind] crate to walk through the stack frames of
//! a debugged process, generating a backtrace with information about function
//! calls, addresses, and names. Stack unwinding is essential for understanding
//! the execution context of a program at a particular point in time.
//!
//! The implementation is inspired by the BugStalker debugger project:
//! <https://github.com/godzie44/BugStalker> (MIT Licensed)

use crate::errors::Result;
use crate::Addr;

use nix::unistd::Pid;
use serde::Serialize;
use unwind::{Accessors, AddressSpace, Byteorder, Cursor, PTraceState, RegNum};

/// Represents a backtrace of the call stack
///
/// [`Backtrace`] contains a list of stack frames, ordered from top (most recent call)
/// to bottom (earliest call), providing a view of the call chain that led to the
/// current execution point.
///
/// # Examples
///
/// ```no_run
/// use coreminer::unwind::unwind;
/// use nix::unistd::Pid;
///
/// // Generate a backtrace for process with PID 1234
/// let backtrace = unwind(Pid::from_raw(1234)).unwrap();
///
/// // Print the backtrace
/// for (i, frame) in backtrace.frames.iter().enumerate() {
///     println!("#{} {} at {:?}", i, frame.name.as_deref().unwrap_or("??"), frame.addr);
/// }
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct Backtrace {
    /// Stack frames in the backtrace
    pub frames: Vec<BacktraceFrame>,
}

/// Represents a single frame in a backtrace
///
/// [`BacktraceFrame`] contains information about a function call in the backtrace,
/// including the current instruction address, function start address, and function name.
///
/// # Examples
///
/// ```
/// use coreminer::unwind::BacktraceFrame;
/// use coreminer::addr::Addr;
///
/// // Create a backtrace frame
/// let frame = BacktraceFrame {
///     addr: Addr::from(0x1000usize),
///     start_addr: Some(Addr::from(0x0F80usize)),
///     name: Some("main".to_string()),
/// };
///
/// // Access frame information
/// println!("Function: {} at {}", frame.name.as_deref().unwrap_or("??"), frame.addr);
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct BacktraceFrame {
    /// Current instruction address
    pub addr: Addr,

    /// Function start address
    pub start_addr: Option<Addr>,

    /// Function name
    pub name: Option<String>,
}

impl Backtrace {
    /// Creates a new backtrace from a list of frames
    ///
    /// # Parameters
    ///
    /// * `frames` - The stack frames to include in the backtrace
    ///
    /// # Returns
    ///
    /// A new [`Backtrace`] instance with the specified frames
    fn new(frames: &[BacktraceFrame]) -> Self {
        Self {
            frames: frames.to_vec(),
        }
    }
}

/// Generates a [Backtrace] for the specified process
///
/// This function walks the call stack of the target process, collecting
/// information about each stack frame to generate a complete backtrace.
///
/// # Parameters
///
/// * `pid` - The process ID of the target process
///
/// # Returns
///
/// * `Ok(Backtrace)` - The generated backtrace
/// * `Err(DebuggerError)` - If unwinding failed
///
/// # Errors
///
/// This function can fail if:
/// - The process cannot be accessed
/// - The stack is corrupted or cannot be unwound
/// - Register access fails
///
/// # Examples
///
/// ```no_run
/// use coreminer::unwind::unwind;
/// use nix::unistd::Pid;
///
/// // Generate a backtrace for process with PID 1234
/// match unwind(Pid::from_raw(1234)) {
///     Ok(backtrace) => {
///         for frame in backtrace.frames {
///             println!("{:?} - {}", frame.addr, frame.name.unwrap_or_else(|| "??".to_string()));
///         }
///     },
///     Err(e) => println!("Failed to generate backtrace: {}", e),
/// }
/// ```
pub fn unwind(pid: Pid) -> Result<Backtrace> {
    let state = PTraceState::new(pid.as_raw() as u32)?;
    let address_space = AddressSpace::new(Accessors::ptrace(), Byteorder::DEFAULT)?;
    let mut cursor = Cursor::remote(&address_space, &state)?;
    let mut frames = vec![];

    loop {
        let ip = cursor.register(RegNum::IP)?;
        match (cursor.procedure_info(), cursor.procedure_name()) {
            (Ok(ref info), Ok(ref name)) if ip == info.start_ip() + name.offset() => {
                let fn_name = format!("{:#}", rustc_demangle::demangle(name.name()));

                frames.push(BacktraceFrame {
                    name: Some(fn_name),
                    start_addr: Some(info.start_ip().into()),
                    addr: ip.into(),
                });
            }
            _ => {
                frames.push(BacktraceFrame {
                    name: None,
                    start_addr: None,
                    addr: ip.into(),
                });
            }
        }

        if !cursor.step()? {
            break;
        }
    }

    Ok(Backtrace::new(&frames))
}

use std::str::FromStr;

use dialoguer::BasicHistory;
use tracing::{error, info, trace, warn};

use super::{DebuggerUI, Register, Status};
use crate::errors::Result;
use crate::feedback::Feedback;
use crate::Addr;

pub struct CliUi {
    buf: String,
    buf_preparsed: Vec<String>,
    history: BasicHistory,
    stepper: usize,
}

impl CliUi {
    pub fn build() -> Result<Self> {
        let ui = CliUi {
            buf_preparsed: Vec::new(),
            buf: String::new(),
            history: BasicHistory::new(),
            stepper: 0,
        };
        Ok(ui)
    }

    pub fn get_input(&mut self) -> Result<()> {
        self.buf = dialoguer::Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .history_with(&mut self.history)
            .interact_text()?;
        trace!("processing '{}'", self.buf);
        self.buf_preparsed = self.buf.split_whitespace().map(|a| a.to_string()).collect();
        trace!("preparsed: {:?}", self.buf_preparsed);
        Ok(())
    }
}

// Für alle values die wir brauchen
//  1. abfragen von basis infos
//  2. coreminer macht was mit der abfrage
//  3. process wird wieder mit dem passenden feedback gecalled
//  4. infos updaten
// 5. ERST JETZT UI

impl DebuggerUI for CliUi {
    fn process(&mut self, feedback: Feedback) -> crate::errors::Result<Status> {
        if let Feedback::Error(e) = feedback {
            warn!("{e}");
        } else if let Feedback::Text(t) = feedback {
            info!("\n{t}");
        } else if let Feedback::Disassembly(d) = feedback {
            info!("\n{d}");
        } else {
            info!("{feedback}");
        }

        if self.stepper > 0 {
            self.stepper -= 1;
            return Ok(Status::StepSingle);
        }

        loop {
            self.get_input()?;

            if string_matches(&self.buf_preparsed[0], &["cont", "c"]) {
                return Ok(Status::Continue);
            } else if string_matches(&self.buf_preparsed[0], &["delbreak", "dbp"]) {
                let addr_raw: usize = get_number(&self.buf_preparsed[1])? as usize;
                let addr: Addr = Addr::from(addr_raw);
                return Ok(Status::DelBreakpoint(addr));
            } else if string_matches(&self.buf_preparsed[0], &["d", "dis"]) {
                if self.buf_preparsed.len() < 3 {
                    error!("d ADDR LEN");
                    continue;
                }
                let addr_raw: usize = get_number(&self.buf_preparsed[1])? as usize;
                let addr = Addr::from(addr_raw);
                let len: usize = get_number(&self.buf_preparsed[2])? as usize;
                return Ok(Status::DisassembleAt(addr, len));
            } else if string_matches(&self.buf_preparsed[0], &["break", "bp"]) {
                let addr_raw: usize = get_number(&self.buf_preparsed[1])? as usize;
                let addr: Addr = Addr::from(addr_raw);
                return Ok(Status::SetBreakpoint(addr));
            } else if string_matches(&self.buf_preparsed[0], &["set"]) {
                if self.buf_preparsed.len() < 3 {
                    error!("sym CMD ARG");
                    continue;
                }
                if self.buf_preparsed[1] == "stepper" {
                    let steps: usize = get_number(&self.buf_preparsed[2])? as usize;
                    self.stepper = steps;
                } else {
                    error!("unknown subcommand")
                }
                continue;
            } else if string_matches(&self.buf_preparsed[0], &["sym", "gsym"]) {
                if self.buf_preparsed.len() < 2 {
                    error!("sym SYMBOL");
                    continue;
                }
                let symbol_name: String = self.buf_preparsed[1].to_string();
                return Ok(Status::GetSymbolsByName(symbol_name));
            } else if string_matches(&self.buf_preparsed[0], &["bt"]) {
                return Ok(Status::Backtrace);
            } else if string_matches(&self.buf_preparsed[0], &["so"]) {
                return Ok(Status::StepOut);
            } else if string_matches(&self.buf_preparsed[0], &["su", "sov"]) {
                return Ok(Status::StepOver);
            } else if string_matches(&self.buf_preparsed[0], &["si"]) {
                return Ok(Status::StepInto);
            } else if string_matches(&self.buf_preparsed[0], &["s", "step"]) {
                return Ok(Status::StepSingle);
            } else if string_matches(&self.buf_preparsed[0], &["info"]) {
                return Ok(Status::Infos);
            } else if string_matches(&self.buf_preparsed[0], &["rmem"]) {
                if self.buf_preparsed.len() < 2 {
                    error!("rmem ADDR");
                    continue;
                }
                let addr_raw: usize = get_number(&self.buf_preparsed[1])? as usize;
                let addr: Addr = Addr::from(addr_raw);
                return Ok(Status::ReadMem(addr));
            } else if string_matches(&self.buf_preparsed[0], &["wmem"]) {
                if self.buf_preparsed.len() < 3 {
                    error!("wmem ADDR VAL");
                    continue;
                }
                let addr_raw: usize = get_number(&self.buf_preparsed[1])? as usize;
                let addr: Addr = Addr::from(addr_raw);
                let value: i64 = get_number(&self.buf_preparsed[1])? as i64;
                return Ok(Status::WriteMem(addr, value));
            } else if string_matches(&self.buf_preparsed[0], &["regs"]) {
                if self.buf_preparsed.len() < 2 {
                    error!("need to give a subcommand");
                    continue;
                }
                if self.buf_preparsed[1] == "get" {
                    return Ok(Status::DumpRegisters);
                } else if self.buf_preparsed[1] == "set" {
                    if self.buf_preparsed.len() != 4 {
                        error!("regs set REGISTER VALUE");
                        continue;
                    }
                    let register = Register::from_str(&self.buf_preparsed[2])?;
                    let value: u64 = get_number(&self.buf_preparsed[1])?;
                    return Ok(Status::SetRegister(register, value));
                } else {
                    error!("only set and get is possible")
                }
            } else {
                error!("bad input, use help if we already bothered to implement that");
            }
        }
    }
}

fn string_matches(cmd: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|a| cmd == *a)
}

fn get_number(mut raw: &str) -> Result<u64> {
    if raw.starts_with("0x") {
        raw = raw.strip_prefix("0x").unwrap();
    }

    Ok(u64::from_str_radix(raw, 16)?)
}

#[cfg(test)]
mod test {
    use crate::ui::cli::get_number;

    #[test]
    fn test_get_number() {
        assert_eq!(0x19u64, get_number("19").unwrap());
        assert_eq!(0x19u64, get_number("0x19").unwrap());
        assert_eq!(0x19u64, get_number("0x00019").unwrap());
        assert_eq!(0x19u64, get_number("00019").unwrap());
        assert_eq!(0x19usize, get_number("19").unwrap() as usize);
    }
}

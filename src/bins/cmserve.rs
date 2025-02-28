use coreminer::debugger::Debugger;
use coreminer::errors::DebuggerError;
use coreminer::ui::json::JsonUI;

use clap::Parser;
use tracing::debug;

/// Launch the core debugger
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {}

fn main() -> Result<(), DebuggerError> {
    setup_logger();
    debug!("set up the logger");

    let _args = Args::parse();

    let ui = JsonUI::build()?;
    let mut debug: Debugger<_> = Debugger::build(ui)?;
    debug.run_debugger()?;
    debug.cleanup()?;

    Ok(())
}

fn setup_logger() {
    // construct a subscriber that prints formatted traces to stdout
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .without_time()
        .finish();
    // use that subscriber to process traces emitted after this point
    tracing::subscriber::set_global_default(subscriber).expect("could not setup logger");
}

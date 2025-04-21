use clap::CommandFactory as _;
use cmd3::console::{Command, Console, ConsoleState};

/// Uppercases stdin.
#[derive(clap::Parser, Debug)]
struct UpperArgs;

struct UpperCommand;

impl Command for UpperCommand {
    fn get_name(&self) -> String {
        "upper".to_string()
    }

    fn get_parser(&self) -> clap::Command {
        UpperArgs::command()
    }

    fn execute(
        &self,
        _state: &mut ConsoleState,
        _args: clap::ArgMatches,
        stdin: &str,
        stdout: &mut dyn std::fmt::Write,
    ) -> Result<(), String> {
        write!(stdout, "{}", stdin.to_uppercase()).map_err(|e| format!("IO error {}", e))?;
        Ok(())
    }
}

struct BuzzCommand;

impl Command for BuzzCommand {
    fn get_name(&self) -> String {
        "buzz".to_string()
    }

    fn get_parser(&self) -> clap::Command {
        /// Registers an asynchronous message with the console.
        #[derive(clap::Parser)]
        struct BuzzArgs {
            /// Message to add to the queue.
            message: String,
        }
        BuzzArgs::command()
    }

    fn execute(
        &self,
        state: &mut ConsoleState,
        _args: clap::ArgMatches,
        _stdin: &str,
        stdout: &mut dyn std::fmt::Write,
    ) -> Result<(), String> {
        state.add_async_message("Bzz bzz...".to_string());
        writeln!(stdout, "Hello, world!").unwrap();
        Ok(())
    }
}

fn main() {
    let mut console = Console::default()
        .add_command(&BuzzCommand {})
        .add_command(&UpperCommand {});

    if let Err(e) = console.cmd_loop() {
        eprintln!("{}", e);
    }
}

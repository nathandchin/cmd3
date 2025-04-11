use clap::CommandFactory as _;
use cmd3::console::{Command, Console};

#[derive(clap::Parser, Debug)]
struct DirArgs {
    path: Option<String>,
}

struct DirCommand;

impl Command for DirCommand {
    fn get_name(&self) -> String {
        "dir".to_string()
    }

    fn get_parser(&self) -> clap::Command {
        DirArgs::command()
    }

    fn execute(&self, args: clap::ArgMatches) -> Result<(), &str> {
        let args: DirArgs = clap::FromArgMatches::from_arg_matches(&args).unwrap();

        dbg!(args);

        Ok(())
    }
}

fn main() {
    let mut console = Console::default().add_command(&DirCommand {});

    if let Err(e) = console.cmd_loop() {
        eprintln!("{}", e);
    }
}

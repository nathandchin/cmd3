use clap::CommandFactory as _;
use cmd3::console::{Command, Console};

/// Outputs "(dir <path>)", where `path` is provided on the command line or via
/// stdin.
#[derive(clap::Parser, Debug)]
struct DirArgs {
    path: Option<String>,

    /// Print output at the end
    #[arg(short = 'v', long)]
    verbose: bool,
}

struct DirCommand;

impl Command for DirCommand {
    fn get_name(&self) -> String {
        "dir".to_string()
    }

    fn get_parser(&self) -> clap::Command {
        DirArgs::command()
    }

    fn execute(
        &self,
        args: clap::ArgMatches,
        stdin: &str,
        stdout: &mut dyn std::fmt::Write,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let args: DirArgs = clap::FromArgMatches::from_arg_matches(&args).unwrap();

        let path = if !stdin.is_empty() {
            stdin
        } else if args.path.is_some() {
            &args.path.unwrap()
        } else {
            return Err("No path provided as argument or from stdin".into());
        };

        if args.verbose {
            write!(stdout, "(dir {})", path)?;
        }

        Ok(())
    }
}

/// Write `arg`s separated by a single space and followed by a newline.
#[derive(clap::Parser, Debug)]
struct EchoArgs {
    /// Arguments to write
    arg: Vec<String>,

    /// Do not append a newline
    #[arg(short = 'n')]
    no_newline: bool,
}

struct EchoCommand;

impl Command for EchoCommand {
    fn get_name(&self) -> String {
        "echo".to_string()
    }

    fn get_parser(&self) -> clap::Command {
        EchoArgs::command()
    }

    fn execute(
        &self,
        args: clap::ArgMatches,
        _stdin: &str,
        stdout: &mut dyn std::fmt::Write,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let args: EchoArgs = clap::FromArgMatches::from_arg_matches(&args).unwrap();

        if args.no_newline {
            write!(stdout, "{}", args.arg.join(" "))?;
        } else {
            write!(stdout, "{}\n", args.arg.join(" "))?;
        }

        Ok(())
    }
}

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
        _args: clap::ArgMatches,
        stdin: &str,
        stdout: &mut dyn std::fmt::Write,
    ) -> Result<(), Box<dyn std::error::Error>> {
        write!(stdout, "{}", stdin.to_uppercase())?;
        Ok(())
    }
}

fn main() {
    let mut console = Console::default()
        .add_command(Box::new(DirCommand {}))
        .add_command(Box::new(EchoCommand {}))
        .add_command(Box::new(UpperCommand {}));

    if let Err(e) = console.cmd_loop() {
        eprintln!("{}", e);
    }
}

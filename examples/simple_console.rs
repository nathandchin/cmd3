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

    fn execute(&self, args: clap::ArgMatches, stdin: Option<&str>) -> Result<Option<String>, &str> {
        let args: DirArgs = clap::FromArgMatches::from_arg_matches(&args).unwrap();

        let path = if let Some(path) = stdin {
            path
        } else if args.path.is_some() {
            &args.path.unwrap()
        } else {
            return Err("No path provided as argument or from stdin");
        };

        let res = format!("(dir {})", path);
        if args.verbose {
            println!("{}", res);
        }

        Ok(Some(res))
    }
}

/// Write `arg`s separated by a single space and followed by a newline.
#[derive(clap::Parser, Debug)]
struct EchoArgs {
    /// Arguments to write
    arg: Vec<String>,
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
        _stdin: Option<&str>,
    ) -> Result<Option<String>, &str> {
        let args: EchoArgs = clap::FromArgMatches::from_arg_matches(&args).unwrap();
        let mut output = args.arg.join(" ");
        output.push('\n');
        Ok(Some(output))
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
        stdin: Option<&str>,
    ) -> Result<Option<String>, &str> {
        Ok(Some(stdin.ok_or("Foo")?.to_uppercase()))
    }
}

fn main() {
    let mut console = Console::default()
        .add_command(&DirCommand {})
        .add_command(&EchoCommand {})
        .add_command(&UpperCommand {});

    if let Err(e) = console.cmd_loop() {
        eprintln!("{}", e);
    }
}

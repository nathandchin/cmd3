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

fn main() {
    let mut console = Console::default().add_command(&DirCommand {});

    if let Err(e) = console.cmd_loop() {
        eprintln!("{}", e);
    }
}

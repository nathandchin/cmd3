use std::collections::HashMap;

use rustyline::error::ReadlineError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConsoleError {
    #[error("Uncategorized")]
    Uncategorized,
    #[error("Error with readline: {0}")]
    ReadlineError(ReadlineError),
    #[error("Error splitting string: {0}")]
    LexingError(String),
    #[error("Error: empty command")]
    EmptyCommandLineError,
    #[error("Unrecognized command: `{0}`")]
    UnrecognizedCommand(String),
    #[error("Error executing command: {0}")]
    CommandError(String),
}

pub trait Command {
    fn get_name(&self) -> String;

    // It would be nice to return a `dyn clap::FromArgMatches` or `dyn
    // clap::Parser` here, but neither of those are `dyn` safe, so we settle for
    // `clap::Command`
    fn get_parser(&self) -> clap::Command;

    // A generic `ArgMatches` is the best we can do, so it's up to the
    // implementor to convert `args` to their desired type.
    fn execute(
        &self,
        args: clap::ArgMatches,
        stdin: Option<&str>,
        stdout: &mut dyn std::fmt::Write,
    ) -> Result<(), String>;
}

pub struct Console<'a> {
    prompt: String,
    commands: HashMap<String, &'a dyn Command>,
}

impl<'a> Console<'a> {
    pub fn cmd_loop(&mut self) -> Result<(), ConsoleError> {
        let mut rl = rustyline::DefaultEditor::new()?;
        'command_loop: loop {
            let readline = match rl.readline(&self.prompt) {
                Ok(o) => o,
                Err(e) => match e {
                    ReadlineError::Eof => return Ok(()),
                    _ => return Err(ConsoleError::from(e)),
                },
            };

            let command_lines = readline.split("|").collect::<Vec<_>>();
            let mut commands: Vec<(&dyn Command, clap::ArgMatches)> = vec![];

            /*
             * First, parse every command in the pipeline. If one fails, then
             * the pipeline shouldn't run at all.
             */
            for command_line in command_lines {
                let tokens = shlex::split(command_line)
                    .ok_or_else(|| ConsoleError::LexingError(command_line.to_string()))?;

                if tokens.is_empty() {
                    eprintln!("{}", ConsoleError::EmptyCommandLineError);
                    continue 'command_loop;
                }

                if let Some(cmd) = self.commands.get(&tokens[0]) {
                    let matches = match cmd.get_parser().try_get_matches_from(&tokens) {
                        Ok(matches) => matches,
                        Err(e) => {
                            eprintln!("{}", e);
                            continue 'command_loop;
                        }
                    };
                    commands.push((*cmd, matches));
                } else {
                    eprintln!("{}", ConsoleError::UnrecognizedCommand(tokens[0].clone()));
                    continue 'command_loop;
                }
            }

            /*
             * Now that we know each command exists and has appropriate
             * arguments, run them in series and pass the output from each to
             * the next.
             */
            let mut previous_output = String::new();
            for (command, args) in commands {
                let mut output_buf = String::new();

                let stdin = if previous_output.is_empty() {
                    None
                } else {
                    Some(previous_output.as_str())
                };

                match command.execute(args, stdin, &mut output_buf) {
                    Ok(_) => {
                        std::mem::swap(&mut previous_output, &mut output_buf);
                    }
                    Err(e) => eprintln!("{}", e),
                }
            }

            /*
             * Print the output at the end of the pipeline
             */
            println!("{}", previous_output);
        }
    }

    pub fn add_command(mut self, cmd: &'a dyn Command) -> Self {
        self.commands.insert(cmd.get_name(), cmd);
        self
    }
}

impl Default for Console<'_> {
    fn default() -> Self {
        Self {
            prompt: "> ".to_string(),
            commands: HashMap::new(),
        }
    }
}

impl From<ReadlineError> for ConsoleError {
    fn from(value: ReadlineError) -> Self {
        Self::ReadlineError(value)
    }
}

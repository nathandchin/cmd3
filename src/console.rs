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
    fn execute(&self, args: clap::ArgMatches, stdin: Option<&str>) -> Result<Option<String>, &str>;
}

pub struct Console<'a> {
    prompt: String,
    commands: HashMap<String, &'a dyn Command>,
}

impl<'a> Console<'a> {
    pub fn cmd_loop(&mut self) -> Result<(), ConsoleError> {
        let mut rl = rustyline::DefaultEditor::new()?;
        loop {
            let readline = match rl.readline(&self.prompt) {
                Ok(o) => o,
                Err(e) => match e {
                    ReadlineError::Eof => return Ok(()),
                    _ => return Err(ConsoleError::from(e)),
                },
            };

            let commands = readline.split("|").collect::<Vec<_>>();

            let mut previous_output = None;
            let mut previous_output_buf;
            for command in commands {
                let tokens = shlex::split(command)
                    .ok_or_else(|| ConsoleError::LexingError(command.to_string()))?;

                if tokens.is_empty() {
                    continue;
                }

                if let Some(cmd) = self.commands.get(&tokens[0]) {
                    let matches = match cmd.get_parser().try_get_matches_from(&tokens) {
                        Ok(matches) => matches,
                        Err(e) => {
                            eprintln!("{}", e);
                            continue;
                        }
                    };

                    let result = cmd.execute(matches, previous_output);

                    match result {
                        Ok(output) => {
                            previous_output = match output {
                                Some(s) => {
                                    previous_output_buf = s;
                                    Some(&previous_output_buf)
                                }
                                None => None,
                            }
                        }
                        Err(e) => eprintln!("{}", e),
                    }
                } else {
                    eprintln!("Unrecognized command")
                }
            }
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

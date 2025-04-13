use std::{collections::HashMap, io::Write as _};

use rustyline::{
    completion::{Completer, Pair},
    error::ReadlineError,
    Completer, Helper, Highlighter, Hinter, Validator,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConsoleError {
    #[error("Uncategorized")]
    Uncategorized,
    #[error("Error with readline: {0}")]
    ReadlineError(ReadlineError),
    #[error("Error writing to stdout")]
    StdoutWriteError,
    #[error("Error splitting string: {0}")]
    LexingError(String),
    #[error("Error: empty command")]
    EmptyCommandLineError,
    #[error("Unrecognized command: `{0}`")]
    UnrecognizedCommand(String),
    #[error("Error executing command: {0}")]
    CommandError(String),
    #[error("Pipeline broken: {0}")]
    BrokenPipeError(Box<ConsoleError>),
}

#[derive(Helper, Completer, Validator, Hinter, Highlighter)]
struct ConsoleHelper<'a> {
    #[rustyline(Completer)]
    completer: CommandCompleter<'a>,
}

struct CommandCompleter<'a> {
    commands: &'a HashMap<String, &'a dyn Command>,
}

impl<'a> CommandCompleter<'a> {
    fn new(commands: &'a HashMap<String, &'a dyn Command>) -> Self {
        Self { commands }
    }
}

impl Completer for CommandCompleter<'_> {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let subtokens = match shlex::split(&line[0..pos]) {
            Some(o) => o,
            None => return Ok((pos, vec![])),
        };

        let is_first_word =
            subtokens.len() < 2 && !line[0..pos].contains(|o: char| o.is_whitespace());

        if is_first_word {
            let mut res = vec![];
            for command in self.commands.keys() {
                if command.starts_with(line) {
                    res.push(Pair {
                        display: command.to_string(),
                        replacement: command.to_string(),
                    });
                }
            }

            Ok((pos - line.len(), res))
        } else {
            let command = match self.commands.get(&subtokens[0]) {
                Some(c) => *c,
                None => return Ok((pos, vec![])),
            };

            let mut completions: Vec<Pair> = vec![];

            let parser = command.get_parser();
            if line.chars().nth(pos - 1).unwrap().is_whitespace() {
                // No current word, show all positional args
                for arg in parser.get_positionals() {
                    completions.push(Pair {
                        display: arg.get_id().to_string(),
                        replacement: "".to_string(), // Don't actually complete these metavars
                    });
                }
                Ok((pos, completions))
            } else {
                let word = subtokens.last().unwrap();

                if word.starts_with("--") {
                    for arg in parser.get_opts() {
                        let id = arg.get_id();
                        let repl = format!("--{}", id);
                        if repl.starts_with(word) {
                            completions.push(Pair {
                                display: format!("[{}]", repl),
                                replacement: repl,
                            });
                        }
                    }
                    Ok((pos - word.len(), completions))
                } else if word.starts_with("-") {
                    for arg in parser.get_opts() {
                        let id = arg.get_id();
                        let (display, replacement) = if let Some(short) = arg.get_short() {
                            (format!("[-{}, --{}]", short, id), format!("-{} ", short))
                        } else {
                            (format!("[--{}]", id), "".to_string())
                        };

                        if replacement.starts_with(word) {
                            completions.push(Pair {
                                display,
                                replacement,
                            });
                        }
                    }
                    Ok((pos - 2, completions))
                } else {
                    Ok((pos, vec![]))
                }
            }
        }
    }
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
        let rl_config = rustyline::Config::builder()
            .check_cursor_position(true) // Prevent overwriting of stdout
            .completion_type(rustyline::CompletionType::List)
            .build();
        let mut rl = rustyline::Editor::with_config(rl_config)?;
        rl.set_helper(Some(ConsoleHelper {
            completer: CommandCompleter::new(&self.commands),
        }));

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

            let in_pipeline = commands.len() > 1;

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
                    Err(e) => {
                        let mut error = ConsoleError::CommandError(e);

                        // If this is a pipeline of multiple commands, then wrap
                        // the current command's error in a pipeline error.
                        if in_pipeline {
                            error = ConsoleError::BrokenPipeError(Box::new(error));
                        }

                        eprintln!("{}", error);
                        continue 'command_loop;
                    }
                }
            }

            /*
             * Print the output at the end of the pipeline
             */
            print!("{}", previous_output);
            std::io::stdout()
                .flush()
                .map_err(|_| ConsoleError::StdoutWriteError)?;
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

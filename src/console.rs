use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    fmt::Write as _,
    io::Write as _,
    process::Stdio,
    rc::Rc,
};

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
    #[error("Error lexing string: {0}")]
    LexingError(String),
    #[error("Error: empty command")]
    EmptyCommandLineError,
    #[error("Unrecognized command: `{0}`")]
    UnrecognizedCommand(String),
    #[error("Error executing command `{0}`: {1}")]
    CommandError(String, String),
    #[error("Pipeline broken: {0}")]
    BrokenPipeError(Box<ConsoleError>),
}

#[derive(Helper, Completer, Validator, Hinter, Highlighter)]
struct ConsoleHelper {
    #[rustyline(Completer)]
    completer: CommandCompleter,
}

struct CommandCompleter {
    parsers: HashMap<String, clap::Command>,
}

impl CommandCompleter {
    fn new(parsers: HashMap<String, clap::Command>) -> Self {
        Self { parsers }
    }
}

impl Completer for CommandCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let orig_pos = pos;
        let (line, pos) = if let Some(i) = line.rfind('|') {
            (&line[i + 1..], pos - i - 1)
        } else {
            (line, pos)
        };

        let subtokens = match shlex::split(&line[0..pos]) {
            Some(o) => o,
            None => return Ok((pos, vec![])),
        };

        let is_first_word =
            subtokens.len() < 2 && !line[0..pos].contains(|o: char| o.is_whitespace());

        if is_first_word {
            // We are completing the name of a command
            let mut res = vec![];
            for command in self.parsers.keys() {
                if command.starts_with(line) {
                    res.push(Pair {
                        display: command.to_string(),
                        replacement: command.to_string(),
                    });
                }
            }

            Ok((orig_pos.saturating_sub(line.len()), res))
        } else {
            // We are completing an argument to a command
            let parser = match self.parsers.get(&subtokens[0]) {
                Some(c) => c,
                None => return Ok((orig_pos, vec![])), // Unrecognized command
            };

            let mut completions: Vec<Pair> = vec![];

            if line.chars().nth(pos - 1).unwrap().is_whitespace() {
                // Cursor is not on a word, show all positional args
                for arg in parser.get_positionals() {
                    completions.push(Pair {
                        display: arg.get_id().to_string(),
                        replacement: "".to_string(), // Don't actually complete these metavars
                    });
                }
                Ok((orig_pos, completions))
            } else {
                let word = subtokens.last().unwrap();

                if word.starts_with("--") {
                    // Long form
                    for arg in parser.get_opts() {
                        if let Some(long) = arg.get_long() {
                            // Only one possibility: long form
                            let replacement = format!("--{}", long);

                            if replacement.starts_with(word) {
                                completions.push(Pair {
                                    display: format!("[{}]", replacement),
                                    replacement,
                                });
                            }
                        }
                    }
                    Ok((orig_pos - word.len(), completions))
                } else if word.starts_with("-") {
                    // Short OR long form
                    for arg in parser.get_opts() {
                        let long = arg.get_long();
                        let short = arg.get_short();

                        // Can be any of long+short, long only, or short only
                        let (display, replacement) =
                            if let (Some(long), Some(short)) = (long, short) {
                                (format!("[-{}, --{}]", short, long), format!("-{} ", short))
                            } else if let Some(long) = long {
                                (format!("[--{}]", long), format!("--{} ", long))
                            } else if let Some(short) = short {
                                (format!("[-{}]", short), format!("-{} ", short))
                            } else {
                                // Trying to use such an arg will be a runtime
                                // error when the parser is invoked, but it
                                // won't appear in the result of `get_opts()` so
                                // it doesn't come into play here.
                                unreachable!("Arg must have at least one of long or short form");
                            };

                        if replacement.starts_with(word) {
                            completions.push(Pair {
                                display,
                                replacement,
                            });
                        }
                    }
                    Ok((orig_pos - word.len(), completions))
                } else {
                    // Must be a positional arg, don't bother completing them
                    // since their names are just metavars. Possibly implement
                    // custom completers here?
                    Ok((orig_pos, vec![]))
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
        &mut self,
        args: clap::ArgMatches,
        stdin: &str,
        stdout: &mut dyn std::fmt::Write,
    ) -> Result<(), String>;
}

enum Runnable {
    External {
        name: String,
        args: Vec<String>,
    },
    Command {
        cmd: Rc<RefCell<dyn Command>>,
        args: clap::ArgMatches,
    },
}

pub struct Console {
    prompt: String,
    commands: HashMap<String, Rc<RefCell<dyn Command>>>,
}

fn split_pipeline(pipeline: &str) -> Vec<&str> {
    enum Quote {
        Single,
        Double,
    }

    let mut quote = None;
    let mut command_lines = vec![];
    let mut last_end_idx = 0;
    for (idx, ch) in pipeline.char_indices() {
        match ch {
            '\'' => {
                quote = match quote {
                    Some(kind) => match kind {
                        Quote::Single => None,
                        Quote::Double => Some(Quote::Single),
                    },
                    None => Some(Quote::Single),
                };
            }
            '"' => {
                quote = match quote {
                    Some(kind) => match kind {
                        Quote::Single => Some(Quote::Double),
                        Quote::Double => None,
                    },
                    None => Some(Quote::Double),
                };
            }
            '|' => {
                if quote.is_none() {
                    command_lines.push(&pipeline[last_end_idx..idx]);
                    last_end_idx = idx + 1
                }
            }
            _ => (),
        }
    }
    // Last one
    command_lines.push(&pipeline[last_end_idx..]);

    command_lines
}

impl Console {
    pub fn cmd_loop(&mut self) -> Result<(), ConsoleError> {
        let rl_config = rustyline::Config::builder()
            .check_cursor_position(true) // Prevent overwriting of stdout
            .completion_type(rustyline::CompletionType::List)
            .build();
        let mut rl = rustyline::Editor::with_config(rl_config)?;
        rl.set_helper(Some(ConsoleHelper {
            completer: CommandCompleter::new(
                self.commands
                    .iter()
                    .map(|(name, cmd)| (name.clone(), cmd.borrow().get_parser()))
                    .collect(),
            ),
        }));

        'command_loop: loop {
            let readline = match rl.readline(&self.prompt) {
                Ok(o) => o,
                Err(e) => match e {
                    ReadlineError::Eof => return Ok(()),
                    _ => return Err(ConsoleError::from(e)),
                },
            };

            let command_lines = split_pipeline(&readline);
            let mut runnables: VecDeque<Runnable> = VecDeque::new();

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

                // Handle possible external commands, prefixed by !
                let (external_cmd, rest) = if tokens[0] == "!" {
                    // Standalone '!'
                    (tokens.get(1).map(|s| s.as_str()), &tokens[2..])
                } else if tokens[0].chars().nth(0).is_some_and(|c| c == '!') {
                    // Command starts with '!'
                    (tokens.first().map(|s| &s[1..]), &tokens[1..])
                } else {
                    // No '!'
                    (None, &[] as &[String])
                };

                if let Some(program) = external_cmd {
                    runnables.push_back(Runnable::External {
                        name: program.to_string(),
                        args: rest.to_vec(),
                    });
                } else if let Some(cmd) = self.commands.get(&tokens[0]) {
                    let matches = match cmd.borrow().get_parser().try_get_matches_from(&tokens) {
                        Ok(matches) => matches,
                        Err(e) => {
                            eprintln!("{}", e);
                            continue 'command_loop;
                        }
                    };

                    runnables.push_back(Runnable::Command {
                        cmd: Rc::clone(cmd),
                        args: matches,
                    });
                } else {
                    eprintln!("{}", ConsoleError::UnrecognizedCommand(tokens[0].clone()));
                    continue 'command_loop;
                }
            }

            let in_pipeline = runnables.len() > 1;

            /*
             * Now that we know each command exists and has appropriate
             * arguments, run them in series and pass the output from each to
             * the next.
             */
            let mut previous_output = String::new();
            while let Some(runnable) = runnables.pop_front() {
                let mut output_buf = String::new();
                let (res, command_name) = match runnable {
                    Runnable::External { name, args } => (
                        Self::run_external_command(
                            &name,
                            &args.iter().map(|s| s.as_str()).collect(),
                            &previous_output,
                            &mut output_buf,
                        ),
                        name,
                    ),
                    Runnable::Command { cmd, args } => {
                        let cmd = &mut *cmd.borrow_mut();
                        (
                            cmd.execute(args, &previous_output, &mut output_buf),
                            cmd.get_name(),
                        )
                    }
                };

                if let Err(error_msg) = res {
                    let mut error = ConsoleError::CommandError(command_name, error_msg);

                    // If this is a pipeline of multiple commands, then wrap
                    // the current command's error in a pipeline error.
                    if in_pipeline {
                        error = ConsoleError::BrokenPipeError(Box::new(error));
                    }

                    eprintln!("{}", error);
                    continue 'command_loop;
                }

                std::mem::swap(&mut previous_output, &mut output_buf);
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

    fn run_external_command(
        name: &str,
        args: &Vec<&str>,
        stdin: &str,
        stdout: &mut String,
    ) -> Result<(), String> {
        /*
         * There are a lot of `expect()`s here. Maybe at some point these can be
         * handled, but for now they are outside the scope of an
         * user-interactive application.
         */

        let mut child = std::process::Command::new(name)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            // .map_err(|e| e.to_string())?;
            .map_err(|e| e.to_string())?;

        let mut child_stdin = child
            .stdin
            .take()
            .expect("Could not acquire stdin for child process");

        std::thread::scope(|s| {
            s.spawn(move || child_stdin.write_all(stdin.as_bytes()))
                .join()
                .expect("Panic while writing to child process stdin")
        })
        .expect("io error while writing to child process stdin");

        let output = child.wait_with_output().expect("TODO");

        // This avoids the pipeline and just goes to the console process's
        // stderr.
        eprint!("{}", String::from_utf8_lossy(&output.stderr));

        write!(stdout, "{}", String::from_utf8_lossy(&output.stdout))
            .map_err(|e| format!("IO error {}", e))?;

        Ok(())
    }

    pub fn add_command(mut self, cmd: Rc<RefCell<dyn Command>>) -> Self {
        self.commands
            .insert(cmd.borrow().get_name(), Rc::clone(&cmd));
        self
    }
}

impl Default for Console {
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

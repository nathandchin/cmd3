use std::collections::VecDeque;

use rustyline::completion::{Completer, Pair};

use crate::console::CommandSet;

pub(crate) struct CommandCompleter {
    commands: CommandSet,
}

impl CommandCompleter {
    pub fn new(commands: CommandSet) -> Self {
        Self { commands }
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

        let mut subtokens = VecDeque::from(match shlex::split(&line[0..pos]) {
            Some(o) => o,
            None => return Ok((pos, vec![])),
        });

        let (is_first_word, prefix) = if subtokens.is_empty() {
            (true, "")
        } else {
            (
                subtokens.len() < 2 && !line[0..pos].contains(|o: char| o.is_whitespace()),
                line.trim(),
            )
        };

        let command_set = &self.commands.borrow();

        if is_first_word {
            // We are completing the name of a command
            let mut res = vec![];
            for command in self.commands.borrow().keys() {
                if command.starts_with(prefix) {
                    res.push(Pair {
                        display: command.to_string(),
                        replacement: command.to_string(),
                    });
                }
            }

            Ok((orig_pos.saturating_sub(line.len()), res))
        } else {
            // We are completing an argument to a command
            let command = match command_set.get(&subtokens.pop_front().unwrap_or_default()) {
                Some(c) => c,
                None => return Ok((orig_pos, vec![])), // Unrecognized command
            };

            let mut completions: Vec<Pair> = vec![];
            let parser = command.get_parser();

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
                let word = subtokens.pop_back().unwrap();

                if word.starts_with("--") {
                    // Long form
                    for arg in parser.get_opts() {
                        if let Some(long) = arg.get_long() {
                            // Only one possibility: long form
                            let replacement = format!("--{long}");

                            if replacement.starts_with(&word) {
                                completions.push(Pair {
                                    display: format!("[{replacement}]"),
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
                                (format!("[-{short}, --{long}]"), format!("-{short} "))
                            } else if let Some(long) = long {
                                (format!("[--{long}]"), format!("--{long} "))
                            } else if let Some(short) = short {
                                (format!("[-{short}]"), format!("-{short} "))
                            } else {
                                // Trying to use such an arg will be a runtime
                                // error when the parser is invoked, but it
                                // won't appear in the result of `get_opts()` so
                                // it doesn't come into play here.
                                unreachable!("Arg must have at least one of long or short form");
                            };

                        if replacement.starts_with(&word) {
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

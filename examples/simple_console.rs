use cmd3::console::{Command, Console};

struct DirCommand;

impl Command for DirCommand {
    fn get_name(&self) -> String {
        "dir".to_string()
    }

    fn execute(&self, _arguments: &[String]) -> Result<(), &str> {
        println!("In `dir`!");
        Ok(())
    }
}

fn main() {
    let mut console = Console::default().add_command(&DirCommand {});

    if let Err(e) = console.cmd_loop() {
        eprintln!("{}", e);
    }
}

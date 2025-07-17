# Summary
This library allows developers to quickly create
interactive command line interfaces for applications.

**NOTE**: The API is not yet stable.

This library is heavily inspired by [cmd2](https://github.com/python-cmd2/cmd2) for Python.

# Features
- Quickly define new commands by implementing the `Command` trait
- Tab complete user-defined commands and their arguments
- Call external commands by prefixing them with `!`
- Pipe of output between internal and external commands seamlessly:
  ```
  > fetch-database-results | !column -t
  ```

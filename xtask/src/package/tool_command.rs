use std::fmt;
use std::path::Path;
use std::process::{Command, Output};

use anyhow::{Context, Result, anyhow};

/// A system tool run with its arguments, kept as data so that tests can check what packaging
/// runs without the tools being present.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolCommand {
    pub program: String,
    pub arguments: Vec<String>,
    /// Variables set for this tool on top of the inherited environment.
    pub environment: Vec<(String, String)>,
}

impl ToolCommand {
    pub fn new<I, S>(program: &str, arguments: I) -> ToolCommand
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        ToolCommand {
            program: program.to_owned(),
            arguments: arguments.into_iter().map(Into::into).collect(),
            environment: Vec::new(),
        }
    }

    pub fn with_environment(mut self, name: &str, value: &str) -> ToolCommand {
        self.environment.push((name.to_owned(), value.to_owned()));
        self
    }

    /// Runs the tool and requires it to succeed. An error names the tool, its arguments and its
    /// stderr.
    pub fn run(&self) -> Result<Output> {
        let output = self.output()?;
        if !output.status.success() {
            return Err(self.failure(&output));
        }
        Ok(output)
    }

    /// Runs the tool and returns what it did, successful or not; only a tool that cannot be
    /// started is an error.
    pub fn output(&self) -> Result<Output> {
        Command::new(&self.program)
            .args(&self.arguments)
            .envs(self.environment.iter().map(|(name, value)| (name, value)))
            .output()
            .with_context(|| format!("could not run `{self}`"))
    }

    pub fn failure(&self, output: &Output) -> anyhow::Error {
        anyhow!(
            "`{self}` failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )
    }
}

impl fmt::Display for ToolCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (name, value) in &self.environment {
            write!(formatter, "{name}={value} ")?;
        }
        write!(formatter, "{}", quoted(&self.program))?;
        for argument in &self.arguments {
            write!(formatter, " {}", quoted(argument))?;
        }
        Ok(())
    }
}

fn quoted(text: &str) -> String {
    if text.is_empty() || text.contains(char::is_whitespace) {
        format!("\"{text}\"")
    } else {
        text.to_owned()
    }
}

/// A path as a tool argument. Every path packaging builds comes from UTF-8 arguments and
/// constants, so nothing is lost.
pub fn path_argument(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_failure_names_the_program_arguments_and_stderr() {
        let command = ToolCommand::new("cargo", ["--no-such-flag"]);

        let message = command.run().unwrap_err().to_string();

        assert!(
            message.contains("`cargo --no-such-flag` failed"),
            "{message}"
        );
        assert!(message.contains("--no-such-flag'"), "{message}");
    }

    #[test]
    fn missing_tool_is_an_error_naming_it() {
        let command = ToolCommand::new("no-such-packaging-tool", ["--flag"]);

        let message = format!("{:#}", command.run().unwrap_err());

        assert!(
            message.contains("could not run `no-such-packaging-tool --flag`"),
            "{message}"
        );
    }

    #[test]
    fn display_quotes_arguments_with_spaces_and_shows_the_environment() {
        let command = ToolCommand::new("hdiutil", ["-volname", "Aurora Asset Editor 2.0.7"])
            .with_environment("ARCH", "x86_64");

        assert_eq!(
            command.to_string(),
            "ARCH=x86_64 hdiutil -volname \"Aurora Asset Editor 2.0.7\""
        );
    }
}

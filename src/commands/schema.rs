//! Machine-readable description of the command tree, generated from clap.

use anyhow::Result;
use clap::{Arg, ArgAction, Args, CommandFactory};
use serde_json::{Value, json};

use super::Cli;
use crate::error::{CliError, ErrorKind};

#[derive(Args)]
pub struct SchemaArgs {
    /// Only describe this command (e.g. `index create`)
    pub command: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgType {
    Boolean,
    Integer,
    String,
}

impl ArgType {
    fn as_str(self) -> &'static str {
        match self {
            ArgType::Boolean => "boolean",
            ArgType::Integer => "integer",
            ArgType::String => "string",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ArgSpec {
    /// Argument id, snake_case (also the MCP property name).
    pub name: String,
    pub long: Option<String>,
    pub short: Option<char>,
    pub positional: bool,
    pub required: bool,
    pub ty: ArgType,
    /// Accepts several values.
    pub multiple: bool,
    /// Values are joined with this delimiter when given as one flag.
    pub delimiter: Option<char>,
    pub default: Option<String>,
    pub possible_values: Vec<String>,
    pub env: Option<String>,
    pub help: String,
}

#[derive(Debug, Clone)]
pub struct CommandSpec {
    /// Words after `msc`, e.g. `["index", "create"]`.
    pub path: Vec<String>,
    pub about: String,
    pub args: Vec<ArgSpec>,
    pub subcommands: Vec<CommandSpec>,
}

impl CommandSpec {
    /// Commands that are actually runnable (no subcommands).
    pub fn leaves(&self) -> Vec<&CommandSpec> {
        if self.subcommands.is_empty() {
            return vec![self];
        }
        self.subcommands.iter().flat_map(|c| c.leaves()).collect()
    }

    pub fn find(&self, path: &[String]) -> Option<&CommandSpec> {
        match path.split_first() {
            None => Some(self),
            Some((head, rest)) => self
                .subcommands
                .iter()
                .find(|c| c.path.last() == Some(head))
                .and_then(|c| c.find(rest)),
        }
    }

    pub fn to_json(&self) -> Value {
        let mut v = json!({
            "name": self.path.last().cloned().unwrap_or_else(|| "msc".into()),
            "usage": format!("msc {}", self.path.join(" ")).trim().to_string(),
            "description": self.about,
            "args": self.args.iter().map(ArgSpec::to_json).collect::<Vec<_>>(),
        });
        if !self.subcommands.is_empty() {
            v["subcommands"] = json!(
                self.subcommands
                    .iter()
                    .map(CommandSpec::to_json)
                    .collect::<Vec<_>>()
            );
        }
        v
    }
}

impl ArgSpec {
    pub fn to_json(&self) -> Value {
        let mut v = json!({
            "name": self.name,
            "type": self.ty.as_str(),
            "required": self.required,
            "description": self.help,
        });
        if self.positional {
            v["positional"] = json!(true);
        }
        if let Some(l) = &self.long {
            v["flag"] = json!(format!("--{l}"));
        }
        if let Some(s) = self.short {
            v["short"] = json!(format!("-{s}"));
        }
        if self.multiple {
            v["multiple"] = json!(true);
        }
        if let Some(d) = self.delimiter {
            v["delimiter"] = json!(d.to_string());
        }
        if let Some(d) = &self.default {
            v["default"] = json!(d);
        }
        if !self.possible_values.is_empty() {
            v["enum"] = json!(self.possible_values);
        }
        if let Some(e) = &self.env {
            v["env"] = json!(e);
        }
        v
    }
}

fn arg_spec(arg: &Arg) -> ArgSpec {
    let is_flag = matches!(
        arg.get_action(),
        ArgAction::SetTrue | ArgAction::SetFalse | ArgAction::Count
    );
    let ty = if is_flag {
        ArgType::Boolean
    } else {
        let id = arg.get_value_parser().type_id();
        let ints = [
            clap::builder::ValueParser::from(clap::value_parser!(u64)).type_id(),
            clap::builder::ValueParser::from(clap::value_parser!(usize)).type_id(),
            clap::builder::ValueParser::from(clap::value_parser!(u32)).type_id(),
            clap::builder::ValueParser::from(clap::value_parser!(i64)).type_id(),
        ];
        if ints.contains(&id) {
            ArgType::Integer
        } else {
            ArgType::String
        }
    };
    let multiple = !is_flag
        && (matches!(arg.get_action(), ArgAction::Append)
            || arg.get_value_delimiter().is_some()
            || arg.get_num_args().is_some_and(|n| n.max_values() > 1));
    ArgSpec {
        name: arg.get_id().to_string(),
        long: arg.get_long().map(String::from),
        short: arg.get_short(),
        positional: arg.is_positional(),
        required: arg.is_required_set(),
        ty,
        multiple,
        delimiter: arg.get_value_delimiter(),
        default: (!is_flag)
            .then(|| {
                arg.get_default_values()
                    .first()
                    .map(|d| d.to_string_lossy().into_owned())
            })
            .flatten(),
        possible_values: if is_flag {
            vec![]
        } else {
            arg.get_possible_values()
                .iter()
                .map(|p| p.get_name().to_string())
                .collect()
        },
        env: arg.get_env().map(|e| e.to_string_lossy().into_owned()),
        help: arg.get_help().map(|h| h.to_string()).unwrap_or_default(),
    }
}

fn command_spec(cmd: &clap::Command, path: Vec<String>) -> CommandSpec {
    let args = cmd
        .get_arguments()
        .filter(|a| !a.is_global_set() && !a.is_hide_set())
        .filter(|a| !matches!(a.get_id().as_str(), "help" | "version"))
        .map(arg_spec)
        .collect();
    let subcommands = cmd
        .get_subcommands()
        .filter(|c| c.get_name() != "help")
        .map(|c| {
            let mut p = path.clone();
            p.push(c.get_name().to_string());
            command_spec(c, p)
        })
        .collect();
    CommandSpec {
        path,
        about: cmd.get_about().map(|a| a.to_string()).unwrap_or_default(),
        args,
        subcommands,
    }
}

/// The full command tree.
pub fn root() -> CommandSpec {
    command_spec(&Cli::command(), vec![])
}

/// Options accepted by every command.
pub fn global_args() -> Vec<ArgSpec> {
    Cli::command()
        .get_arguments()
        .filter(|a| a.is_global_set() && !a.is_hide_set())
        .map(arg_spec)
        .collect()
}

pub fn document() -> Value {
    let root = root();
    let exit_codes: Value = std::iter::once(("0".to_string(), json!("Success")))
        .chain(ErrorKind::ALL.iter().map(|k| {
            (
                k.exit_code().to_string(),
                json!(format!("{}: {}", k.as_str(), k.description())),
            )
        }))
        .collect::<serde_json::Map<_, _>>()
        .into();
    json!({
        "name": "msc",
        "version": env!("CARGO_PKG_VERSION"),
        "description": root.about,
        "output": {
            "stdout": "Command result only. Compact JSON when stdout is not a terminal or with --json; NDJSON events for streaming commands.",
            "stderr": "Status messages, and on failure an error object: {\"error\": {\"kind\", \"code\", \"message\", \"exitCode\", \"httpStatus\"?, \"type\"?, \"link\"?, \"hint\"?, \"task\"?}}",
        },
        "exitCodes": exit_codes,
        "environment": {
            "MSC_URL": "Meilisearch URL (bypasses the config file)",
            "MSC_API_KEY": "API key",
            "MSC_PROJECT": "Project name from the config file",
            "MSC_WAIT": "Set to true to wait for tasks on every write",
            "MSC_CONFIG": "Path to the config file (default ~/.config/msc/config.toml)",
            "NO_COLOR": "Disable colored output",
        },
        "globalArgs": global_args().iter().map(ArgSpec::to_json).collect::<Vec<_>>(),
        "commands": root.subcommands.iter().map(CommandSpec::to_json).collect::<Vec<_>>(),
    })
}

pub fn run(args: &SchemaArgs) -> Result<()> {
    if args.command.is_empty() {
        crate::output::print_json(&document());
        return Ok(());
    }
    let root = root();
    let spec = root.find(&args.command).ok_or_else(|| {
        CliError::usage(
            "unknown_command",
            format!("Unknown command: {}", args.command.join(" ")),
        )
        .with_hint("Run `msc schema` to list all commands")
    })?;
    crate::output::print_json(&spec.to_json());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_describes_leaf_commands() {
        let root = root();
        let create = root
            .find(&["index".into(), "create".into()])
            .expect("index create exists");
        let uid = create.args.iter().find(|a| a.name == "uid").unwrap();
        assert!(uid.positional && uid.required);
        let pk = create
            .args
            .iter()
            .find(|a| a.name == "primary_key")
            .unwrap();
        assert_eq!(pk.long.as_deref(), Some("primary-key"));
        assert!(!pk.positional);
    }

    #[test]
    fn schema_detects_types() {
        let root = root();
        let search = root.find(&["search".into()]).unwrap();
        let limit = search.args.iter().find(|a| a.name == "limit").unwrap();
        assert_eq!(limit.ty, ArgType::Integer);
        let facets = search.args.iter().find(|a| a.name == "facets").unwrap();
        assert!(facets.multiple);
        let interactive = search
            .args
            .iter()
            .find(|a| a.name == "interactive")
            .unwrap();
        assert_eq!(interactive.ty, ArgType::Boolean);
    }

    #[test]
    fn globals_are_listed_once() {
        let globals = global_args();
        assert!(globals.iter().any(|a| a.name == "wait"));
        let root = root();
        for leaf in root.leaves() {
            assert!(
                !leaf.args.iter().any(|a| a.name == "wait"),
                "{:?}",
                leaf.path
            );
        }
    }
}

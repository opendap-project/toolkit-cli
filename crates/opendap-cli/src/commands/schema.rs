//! `opendap schema`: command metadata merged with the clap tree.

use clap::CommandFactory;
use opendap_cli_kit::meta::{Category, CommandMeta};
use opendap_cli_kit::output::table;
use opendap_cli_kit::{CliError, OutputMode};
use opendap_model::error as codes;
use serde::Serialize;

use super::{Ctx, Doc};
use crate::cli::Cli;
use crate::meta::{self, COMMANDS};

#[derive(Serialize)]
pub struct ArgDoc {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    long: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    short: Option<char>,
    help: String,
    takes_value: bool,
    required: bool,
    multiple: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    values: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    env: Option<String>,
}

#[derive(Serialize)]
struct ErrorRef {
    code: &'static str,
    exit: u8,
    retryable: bool,
    summary: &'static str,
}

#[derive(Serialize)]
struct CommandDoc {
    command: String,
    path: &'static str,
    summary: &'static str,
    category: Category,
    idempotent: bool,
    targeted: bool,
    output_schema: &'static str,
    arguments: Vec<ArgDoc>,
    flags: Vec<ArgDoc>,
    errors: Vec<ErrorRef>,
    examples: &'static [&'static str],
}

#[derive(Serialize)]
struct All {
    global_flags: Vec<ArgDoc>,
    items: Vec<CommandDoc>,
}

fn arg_doc(a: &clap::Arg) -> ArgDoc {
    // The tree is not built, so num_args is unset; the action says it all.
    let takes_value = a.get_action().takes_values();
    ArgDoc {
        name: a.get_id().to_string(),
        long: a.get_long().map(|l| format!("--{l}")),
        short: a.get_short(),
        help: a.get_help().map(|h| h.to_string()).unwrap_or_default(),
        takes_value,
        required: a.is_required_set(),
        multiple: matches!(a.get_action(), clap::ArgAction::Append),
        values: if takes_value {
            a.get_possible_values()
                .iter()
                .map(|v| v.get_name().to_string())
                .collect()
        } else {
            Vec::new()
        },
        default: a
            .get_default_values()
            .first()
            .map(|v| v.to_string_lossy().into_owned()),
        env: a.get_env().map(|e| e.to_string_lossy().into_owned()),
    }
}

fn find_clap<'a>(root: &'a clap::Command, path: &str) -> Option<&'a clap::Command> {
    path.split(' ')
        .try_fold(root, |c, name| c.find_subcommand(name))
}

fn command_doc(root: &clap::Command, m: &'static CommandMeta) -> CommandDoc {
    let cmd = find_clap(root, m.path).expect("metadata matches the clap tree (tested)");
    let (mut arguments, mut flags) = (Vec::new(), Vec::new());
    for a in cmd.get_arguments().filter(|a| !a.is_global_set()) {
        if a.is_positional() {
            arguments.push(arg_doc(a))
        } else {
            flags.push(arg_doc(a))
        }
    }
    let errors = m
        .errors
        .iter()
        .chain(["input.invalid"].iter())
        .filter_map(|c| codes::lookup(c))
        .map(|c| ErrorRef {
            code: c.code,
            exit: c.exit,
            retryable: c.retryable,
            summary: c.summary,
        })
        .collect();
    CommandDoc {
        command: format!("opendap {}", m.path),
        path: m.path,
        summary: m.summary,
        category: m.category,
        idempotent: m.idempotent,
        targeted: m.targeted,
        output_schema: m.output_schema,
        arguments,
        flags,
        errors,
        examples: m.examples,
    }
}

pub fn run(ctx: &Ctx, words: &[String]) -> Result<u8, CliError> {
    let root = Cli::command();
    let path = words
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let path = path.strip_prefix("opendap ").unwrap_or(&path).to_string();
    if !path.is_empty() {
        let m = meta::find(&path).ok_or_else(|| {
            CliError::new(&codes::COMMAND_UNKNOWN, format!("no command `{path}`"))
                .fix("opendap schema")
        })?;
        let doc = command_doc(&root, m);
        let human = format!(
            "{}\n  {}\n  category: {:?}, idempotent: {}\n  output: {}\n",
            doc.command, m.summary, m.category, m.idempotent, m.output_schema
        );
        ctx.out.emit(
            &Doc {
                schema: "opendap.schema/v1",
                body: &doc,
            },
            || human,
        );
        return Ok(0);
    }
    let global_flags = root
        .get_arguments()
        .filter(|a| a.is_global_set())
        .map(arg_doc)
        .collect();
    let all = All {
        global_flags,
        items: COMMANDS.iter().map(|m| command_doc(&root, m)).collect(),
    };
    let plain = ctx.out.mode == OutputMode::Plain;
    ctx.out.emit(
        &Doc {
            schema: "opendap.schema/v1",
            body: &all,
        },
        || {
            if plain {
                COMMANDS.iter().map(|m| format!("{}\n", m.path)).collect()
            } else {
                let rows: Vec<Vec<String>> = COMMANDS
                    .iter()
                    .map(|m| {
                        vec![
                            m.path.to_string(),
                            format!("{:?}", m.category).to_lowercase(),
                            m.summary.to_string(),
                        ]
                    })
                    .collect();
                table(&["COMMAND", "CATEGORY", "SUMMARY"], &rows)
            }
        },
    );
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_documents_flags_and_errors() {
        let root = Cli::command();
        let doc =
            serde_json::to_value(command_doc(&root, meta::find("vm start").unwrap())).unwrap();
        assert_eq!(doc["command"], "opendap vm start");
        assert_eq!(doc["category"], "mutate");
        assert_eq!(doc["output_schema"], "opendap.vm/v1");
        let flags: Vec<&str> = doc["flags"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["long"].as_str().unwrap())
            .collect();
        assert_eq!(flags, ["--detach", "--wait-timeout"]);
        assert_eq!(doc["arguments"][0]["name"], "name");
        assert_eq!(doc["arguments"][0]["default"], "default");
        let errors: Vec<&str> = doc["errors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["code"].as_str().unwrap())
            .collect();
        assert!(errors.contains(&"vm.image_missing") && errors.contains(&"input.invalid"));

        let key = serde_json::to_value(command_doc(&root, meta::find("input send key").unwrap()))
            .unwrap();
        // A role is the device's to validate (its description names them), so it has no fixed values.
        assert_eq!(key["arguments"][0]["name"], "role");
        assert!(key["arguments"][0]["values"].is_null());
    }

    #[test]
    fn global_flags_are_listed_once() {
        let root = Cli::command();
        let globals: Vec<String> = root
            .get_arguments()
            .filter(|a| a.is_global_set())
            .map(|a| a.get_id().to_string())
            .collect();
        assert_eq!(globals, ["target", "output", "json", "no_input", "dry_run"]);
    }
}

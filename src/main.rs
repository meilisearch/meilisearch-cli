mod client;
mod commands;
mod config;
mod error;
mod output;
mod tui;

use std::process::ExitCode;

use clap::Parser;
use commands::Cli;

#[tokio::main]
async fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => return clap_error(e),
    };
    output::init(output::OutputCtx::new(
        cli.json,
        cli.pretty,
        cli.table,
        cli.quiet,
        cli.select.clone(),
    ));

    match commands::run(cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            let report = error::report(&err);
            // A failed task is still the command's result: put it on stdout so
            // callers can parse it the same way as a successful one.
            if let Some(task) = &report.error.task {
                output::print_json(task);
            }
            if output::is_json() {
                eprintln!("{}", report.to_json());
            } else {
                eprintln!("{}", report.to_human());
            }
            ExitCode::from(report.exit_code())
        }
    }
}

/// Help/version go out as usual; real usage errors become JSON when the
/// caller is a program (piped stdout or an explicit --json).
fn clap_error(e: clap::Error) -> ExitCode {
    use clap::error::ErrorKind as K;
    use std::io::IsTerminal;

    let is_display = matches!(
        e.kind(),
        K::DisplayHelp | K::DisplayVersion | K::DisplayHelpOnMissingArgumentOrSubcommand
    );
    let wants_json = std::env::args()
        .any(|a| matches!(a.as_str(), "--json" | "-j" | "--raw" | "-r"))
        || (!std::io::stdout().is_terminal() && !std::env::args().any(|a| a == "--pretty"));
    if is_display || !wants_json {
        let _ = e.print();
        return ExitCode::from(e.exit_code() as u8);
    }
    let message = e.render().to_string();
    let message = message.trim().trim_start_matches("error: ");
    let code = match e.kind() {
        K::UnknownArgument => "unknown_argument",
        K::InvalidSubcommand => "invalid_subcommand",
        K::MissingRequiredArgument => "missing_argument",
        K::InvalidValue | K::ValueValidation => "invalid_value",
        _ => "invalid_arguments",
    };
    let err = error::CliError::usage(code, message)
        .with_hint("Run `msc schema` or `msc <command> --help` to see valid arguments");
    let report = error::ErrorReport {
        display: err.message.clone(),
        error: err,
    };
    eprintln!("{}", report.to_json());
    ExitCode::from(report.exit_code())
}

use seq_protocol_data::ProtocolRegistry;
use seq_trace::{
    compare_golden, load_golden, load_trace, replay, write_golden, GoldenFile, TraceError,
};
use std::env;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

const USAGE: &str = "\
seq-trace validate <trace.json> [--catalog-dir <dir>]
seq-trace replay <trace.json> [--catalog-dir <dir>] [-o <golden.json>]
seq-trace check <trace.json> <golden.json> [--catalog-dir <dir>]

The build must link the trace backend. Test builds use:
  cargo run -p seq-trace --no-default-features --features backend-test -- ...
";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("seq-trace: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), CliError> {
    let mut args = env::args().skip(1);
    let command = args.next().ok_or(CliError::Usage)?;
    let arguments = args.collect::<Vec<_>>();
    match command.as_str() {
        "validate" => validate_command(&arguments),
        "replay" => replay_command(&arguments),
        "check" => check_command(&arguments),
        "-h" | "--help" | "help" => {
            print!("{USAGE}");
            Ok(())
        }
        _ => Err(CliError::Usage),
    }
}

fn validate_command(arguments: &[String]) -> Result<(), CliError> {
    let parsed = CommonArgs::parse(arguments, false)?;
    if parsed.positionals.len() != 1 {
        return Err(CliError::Usage);
    }
    let trace = load_trace(&parsed.positionals[0])?;
    let registry = load_registry(parsed.catalog_dir.as_deref())?;
    trace.validate_for_registry(&registry)?;
    println!(
        "ok: {} packets, backend {}, catalog {}",
        trace.packets.len(),
        BackendName(trace.backend),
        trace.catalog_hash
    );
    Ok(())
}

fn replay_command(arguments: &[String]) -> Result<(), CliError> {
    let parsed = CommonArgs::parse(arguments, true)?;
    if parsed.positionals.len() != 1 {
        return Err(CliError::Usage);
    }
    let trace = load_trace(&parsed.positionals[0])?;
    let registry = load_registry(parsed.catalog_dir.as_deref())?;
    let result = replay(&trace, registry)?;
    let golden = GoldenFile::from_replay(&trace, &result)?;
    match parsed.output {
        Some(path) => write_golden(
            &golden,
            File::create(&path).map_err(|source| CliError::Create {
                path: path.clone(),
                source,
            })?,
        )?,
        None => write_golden(&golden, std::io::stdout().lock())?,
    }
    Ok(())
}

fn check_command(arguments: &[String]) -> Result<(), CliError> {
    let parsed = CommonArgs::parse(arguments, false)?;
    if parsed.positionals.len() != 2 {
        return Err(CliError::Usage);
    }
    let trace = load_trace(&parsed.positionals[0])?;
    let expected = load_golden(&parsed.positionals[1])?;
    let registry = load_registry(parsed.catalog_dir.as_deref())?;
    let result = replay(&trace, registry)?;
    let actual = GoldenFile::from_replay(&trace, &result)?;
    compare_golden(&expected, &actual).map_err(CliError::Mismatch)?;
    println!("ok: {} exact decode batches match", actual.batches.len());
    Ok(())
}

fn load_registry(catalog_dir: Option<&Path>) -> Result<Arc<ProtocolRegistry>, CliError> {
    let registry = match catalog_dir {
        Some(path) => ProtocolRegistry::from_directory(path)
            .map_err(|error| CliError::Registry(error.to_string()))?,
        None => {
            ProtocolRegistry::embedded().map_err(|error| CliError::Registry(error.to_string()))?
        }
    };
    Ok(Arc::new(registry))
}

struct CommonArgs {
    positionals: Vec<String>,
    catalog_dir: Option<PathBuf>,
    output: Option<PathBuf>,
}

impl CommonArgs {
    fn parse(arguments: &[String], allow_output: bool) -> Result<Self, CliError> {
        let mut positionals = Vec::new();
        let mut catalog_dir = None;
        let mut output = None;
        let mut index = 0;
        while index < arguments.len() {
            match arguments[index].as_str() {
                "--catalog-dir" => {
                    index += 1;
                    let value = arguments.get(index).ok_or(CliError::Usage)?;
                    if catalog_dir.replace(PathBuf::from(value)).is_some() {
                        return Err(CliError::Usage);
                    }
                }
                "-o" | "--output" if allow_output => {
                    index += 1;
                    let value = arguments.get(index).ok_or(CliError::Usage)?;
                    if output.replace(PathBuf::from(value)).is_some() {
                        return Err(CliError::Usage);
                    }
                }
                value if value.starts_with('-') => return Err(CliError::Usage),
                value => positionals.push(value.to_owned()),
            }
            index += 1;
        }
        Ok(Self {
            positionals,
            catalog_dir,
            output,
        })
    }
}

struct BackendName(seq_trace::TraceBackend);

impl std::fmt::Display for BackendName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self.0 {
            seq_trace::TraceBackend::Live => "live",
            seq_trace::TraceBackend::Test => "test",
            seq_trace::TraceBackend::Eql => "eql",
        })
    }
}

#[derive(Debug)]
enum CliError {
    Usage,
    Trace(TraceError),
    Registry(String),
    Create {
        path: PathBuf,
        source: std::io::Error,
    },
    Mismatch(seq_trace::GoldenMismatch),
}

impl From<TraceError> for CliError {
    fn from(value: TraceError) -> Self {
        Self::Trace(value)
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usage => formatter.write_str(USAGE),
            Self::Trace(error) => error.fmt(formatter),
            Self::Registry(error) => write!(formatter, "could not load protocol catalogs: {error}"),
            Self::Create { path, source } => {
                write!(formatter, "could not create {}: {source}", path.display())
            }
            Self::Mismatch(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CliError {}

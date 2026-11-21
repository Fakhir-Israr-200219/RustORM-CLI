use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
#[allow(dead_code)]  // ← ADD THIS — these variants are for future use
pub enum CliError {
    /// Schema parsing failed
    ParseError {
        line: usize,
        column: usize,
        message: String,
    },

    /// Schema validation failed
    ValidationError(Vec<String>),

    /// File I/O error
    IoError {
        path: PathBuf,
        source: std::io::Error,
    },

    /// Code generation failed
    GenerationError(String),

    /// Migration error (reuses runtime MigrationError concept)
    MigrationError(String),

    /// Configuration error
    ConfigError(String),

    /// Database connection failed
    ConnectionError(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::ParseError { line, column, message } => {
                write!(f, "❌ Parse error at line {line}, column {column}: {message}")
            }
            CliError::ValidationError(errors) => {
                writeln!(f, "❌ Schema validation failed:")?;
                for err in errors {
                    writeln!(f, "   • {err}")?;
                }
                Ok(())
            }
            CliError::IoError { path, source } => {
                write!(f, "❌ File error at '{}': {source}", path.display())
            }
            CliError::GenerationError(msg) => {
                write!(f, "❌ Code generation failed: {msg}")
            }
            CliError::MigrationError(msg) => {
                write!(f, "❌ Migration error: {msg}")
            }
            CliError::ConfigError(msg) => {
                write!(f, "❌ Configuration error: {msg}")
            }
            CliError::ConnectionError(msg) => {
                write!(f, "❌ Database connection failed: {msg}")
            }
        }
    }
}

impl std::error::Error for CliError {}

impl From<std::io::Error> for CliError {
    fn from(err: std::io::Error) -> Self {
        CliError::IoError {
            path: PathBuf::from("<unknown>"),
            source: err,
        }
    }
}
impl From<String> for CliError {
    fn from(msg: String) -> Self {
        CliError::GenerationError(msg)
    }
}

pub type Result<T> = std::result::Result<T, CliError>;
//! Raw MongoDB command execution module.
//!
//! This module provides functionality to execute raw MongoDB commands with validation.

use bson::Document;

use crate::connection::pool::ConnectionPool;
use crate::error::MongoCoreError;
use crate::operations::raw_validator::{validate_command, ValidationMode};

/// Options for raw command execution.
#[derive(Debug, Clone)]
pub struct RawCommandOptions {
    /// Validation mode to apply to the command.
    pub validation_mode: ValidationMode,
}

impl Default for RawCommandOptions {
    fn default() -> Self {
        Self {
            validation_mode: ValidationMode::BlockDangerous,
        }
    }
}

/// Executes a raw MongoDB command on the specified database.
///
/// # Arguments
///
/// * `pool` - The connection pool to use for command execution
/// * `database` - The name of the database to run the command against
/// * `command` - The BSON document representing the MongoDB command
/// * `options` - Options controlling command validation and execution
///
/// # Returns
///
/// * `Ok(Document)` - The result document from MongoDB
/// * `Err(MongoCoreError)` - If validation fails or the command execution fails
///
/// # Examples
///
/// ```no_run
/// use bson::doc;
/// use mongocore::connection::pool::ConnectionPool;
/// use mongocore::operations::raw::{run_command, RawCommandOptions};
/// use mongocore::config::Config;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let config = Config::default();
/// let pool = ConnectionPool::connect(&config).await?;
/// let command = doc! { "ping": 1 };
/// let options = RawCommandOptions::default();
///
/// let result = run_command(&pool, "admin", command, &options).await?;
/// # Ok(())
/// # }
/// ```
pub async fn run_command(
    pool: &ConnectionPool,
    database: &str,
    command: Document,
    options: &RawCommandOptions,
) -> Result<Document, MongoCoreError> {
    // Validate the command before execution
    validate_command(&command, &options.validation_mode)?;

    // Get the database handle from the pool
    let db = pool.database(database);

    // Execute the command and map errors to MongoCoreError
    let result = db
        .run_command(command)
        .await
        .map_err(|e| MongoCoreError::OperationError(e.to_string()))?;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bson::doc;

    #[test]
    fn test_raw_command_options_default() {
        let options = RawCommandOptions::default();
        assert_eq!(options.validation_mode, ValidationMode::BlockDangerous);
    }

    #[test]
    fn test_raw_command_options_custom() {
        let options = RawCommandOptions {
            validation_mode: ValidationMode::AllowAll,
        };
        assert_eq!(options.validation_mode, ValidationMode::AllowAll);
    }
}

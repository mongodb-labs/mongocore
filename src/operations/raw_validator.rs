//! Raw MongoDB command validation module.
//!
//! This module provides validation for raw MongoDB commands before execution,
//! allowing blocking of dangerous operations based on configurable validation modes.

use crate::error::MongoCoreError;
use bson::Document;

/// Validation mode for raw MongoDB commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationMode {
    /// Block dangerous administrative commands.
    BlockDangerous,
    /// Allow all commands without validation.
    AllowAll,
}

/// List of dangerous commands that should be blocked in `BlockDangerous` mode.
const DANGEROUS_COMMANDS: &[&str] = &[
    "dropDatabase",
    "dropAllUsersFromDatabase",
    "dropAllRolesFromDatabase",
    "shutdown",
    "replSetReconfig",
    "replSetStepDown",
    "setFeatureCompatibilityVersion",
    "fsync",
    "cleanupOrphaned",
    "compact",
];

/// Validates a raw MongoDB command document against the specified validation mode.
///
/// # Arguments
///
/// * `command` - The BSON document representing the MongoDB command
/// * `mode` - The validation mode to apply
///
/// # Returns
///
/// * `Ok(())` if the command is allowed
/// * `Err(MongoCoreError::ValidationError)` if the command is blocked
///
/// # Examples
///
/// ```no_run
/// use bson::{doc, Document};
/// use mongocore::operations::raw_validator::{validate_command, ValidationMode};
///
/// let command = doc! { "find": "users", "filter": {} };
/// validate_command(&command, &ValidationMode::BlockDangerous).unwrap();
/// ```
pub fn validate_command(command: &Document, mode: &ValidationMode) -> Result<(), MongoCoreError> {
    match mode {
        ValidationMode::AllowAll => Ok(()),
        ValidationMode::BlockDangerous => {
            // Check if any key in the command document matches a dangerous command
            for key in command.keys() {
                if DANGEROUS_COMMANDS.contains(&key.as_str()) {
                    return Err(MongoCoreError::ValidationError(format!(
                        "Command '{}' is blocked by validation policy",
                        key
                    )));
                }
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bson::doc;

    #[test]
    fn test_validation_mode_equality() {
        assert_eq!(
            ValidationMode::BlockDangerous,
            ValidationMode::BlockDangerous
        );
        assert_eq!(ValidationMode::AllowAll, ValidationMode::AllowAll);
        assert_ne!(ValidationMode::BlockDangerous, ValidationMode::AllowAll);
    }

    #[test]
    fn test_allow_all_mode_permits_everything() {
        let mode = ValidationMode::AllowAll;

        // Safe command
        let safe_cmd = doc! { "find": "users", "filter": {} };
        assert!(validate_command(&safe_cmd, &mode).is_ok());

        // Dangerous commands
        let dangerous_cmd = doc! { "dropDatabase": 1 };
        assert!(validate_command(&dangerous_cmd, &mode).is_ok());

        let shutdown_cmd = doc! { "shutdown": 1 };
        assert!(validate_command(&shutdown_cmd, &mode).is_ok());
    }

    #[test]
    fn test_block_dangerous_mode_allows_safe_commands() {
        let mode = ValidationMode::BlockDangerous;

        let safe_commands = vec![
            doc! { "find": "users", "filter": {} },
            doc! { "insert": "users", "documents": [] },
            doc! { "update": "users", "updates": [] },
            doc! { "delete": "users", "deletes": [] },
            doc! { "aggregate": "users", "pipeline": [] },
            doc! { "count": "users" },
            doc! { "distinct": "users", "key": "field" },
            doc! { "findAndModify": "users", "query": {} },
            doc! { "createIndexes": "users", "indexes": [] },
            doc! { "listIndexes": "users" },
            doc! { "listCollections": 1 },
            doc! { "listDatabases": 1 },
        ];

        for cmd in safe_commands {
            assert!(
                validate_command(&cmd, &mode).is_ok(),
                "Safe command should be allowed: {:?}",
                cmd
            );
        }
    }

    #[test]
    fn test_block_dangerous_mode_blocks_drop_database() {
        let mode = ValidationMode::BlockDangerous;
        let cmd = doc! { "dropDatabase": 1 };

        let result = validate_command(&cmd, &mode);
        assert!(result.is_err());

        if let Err(MongoCoreError::ValidationError(msg)) = result {
            assert!(msg.contains("dropDatabase"));
            assert!(msg.contains("blocked by validation policy"));
        } else {
            panic!("Expected ValidationError");
        }
    }

    #[test]
    fn test_block_dangerous_mode_blocks_drop_all_users_from_database() {
        let mode = ValidationMode::BlockDangerous;
        let cmd = doc! { "dropAllUsersFromDatabase": 1 };

        let result = validate_command(&cmd, &mode);
        assert!(result.is_err());

        if let Err(MongoCoreError::ValidationError(msg)) = result {
            assert!(msg.contains("dropAllUsersFromDatabase"));
        } else {
            panic!("Expected ValidationError");
        }
    }

    #[test]
    fn test_block_dangerous_mode_blocks_drop_all_roles_from_database() {
        let mode = ValidationMode::BlockDangerous;
        let cmd = doc! { "dropAllRolesFromDatabase": 1 };

        let result = validate_command(&cmd, &mode);
        assert!(result.is_err());

        if let Err(MongoCoreError::ValidationError(msg)) = result {
            assert!(msg.contains("dropAllRolesFromDatabase"));
        } else {
            panic!("Expected ValidationError");
        }
    }

    #[test]
    fn test_block_dangerous_mode_blocks_shutdown() {
        let mode = ValidationMode::BlockDangerous;
        let cmd = doc! { "shutdown": 1 };

        let result = validate_command(&cmd, &mode);
        assert!(result.is_err());

        if let Err(MongoCoreError::ValidationError(msg)) = result {
            assert!(msg.contains("shutdown"));
        } else {
            panic!("Expected ValidationError");
        }
    }

    #[test]
    fn test_block_dangerous_mode_blocks_repl_set_reconfig() {
        let mode = ValidationMode::BlockDangerous;
        let cmd = doc! { "replSetReconfig": { "config": {} } };

        let result = validate_command(&cmd, &mode);
        assert!(result.is_err());

        if let Err(MongoCoreError::ValidationError(msg)) = result {
            assert!(msg.contains("replSetReconfig"));
        } else {
            panic!("Expected ValidationError");
        }
    }

    #[test]
    fn test_block_dangerous_mode_blocks_repl_set_step_down() {
        let mode = ValidationMode::BlockDangerous;
        let cmd = doc! { "replSetStepDown": 60 };

        let result = validate_command(&cmd, &mode);
        assert!(result.is_err());

        if let Err(MongoCoreError::ValidationError(msg)) = result {
            assert!(msg.contains("replSetStepDown"));
        } else {
            panic!("Expected ValidationError");
        }
    }

    #[test]
    fn test_block_dangerous_mode_blocks_set_feature_compatibility_version() {
        let mode = ValidationMode::BlockDangerous;
        let cmd = doc! { "setFeatureCompatibilityVersion": "5.0" };

        let result = validate_command(&cmd, &mode);
        assert!(result.is_err());

        if let Err(MongoCoreError::ValidationError(msg)) = result {
            assert!(msg.contains("setFeatureCompatibilityVersion"));
        } else {
            panic!("Expected ValidationError");
        }
    }

    #[test]
    fn test_block_dangerous_mode_blocks_fsync() {
        let mode = ValidationMode::BlockDangerous;
        let cmd = doc! { "fsync": 1, "lock": true };

        let result = validate_command(&cmd, &mode);
        assert!(result.is_err());

        if let Err(MongoCoreError::ValidationError(msg)) = result {
            assert!(msg.contains("fsync"));
        } else {
            panic!("Expected ValidationError");
        }
    }

    #[test]
    fn test_block_dangerous_mode_blocks_cleanup_orphaned() {
        let mode = ValidationMode::BlockDangerous;
        let cmd = doc! { "cleanupOrphaned": "test.collection" };

        let result = validate_command(&cmd, &mode);
        assert!(result.is_err());

        if let Err(MongoCoreError::ValidationError(msg)) = result {
            assert!(msg.contains("cleanupOrphaned"));
        } else {
            panic!("Expected ValidationError");
        }
    }

    #[test]
    fn test_block_dangerous_mode_blocks_compact() {
        let mode = ValidationMode::BlockDangerous;
        let cmd = doc! { "compact": "test.collection" };

        let result = validate_command(&cmd, &mode);
        assert!(result.is_err());

        if let Err(MongoCoreError::ValidationError(msg)) = result {
            assert!(msg.contains("compact"));
        } else {
            panic!("Expected ValidationError");
        }
    }

    #[test]
    fn test_all_dangerous_commands_are_blocked() {
        let mode = ValidationMode::BlockDangerous;

        // Ensure every dangerous command in the list is properly blocked
        for &dangerous_cmd in DANGEROUS_COMMANDS {
            let cmd = doc! { dangerous_cmd: 1 };
            let result = validate_command(&cmd, &mode);

            assert!(
                result.is_err(),
                "Command '{}' should be blocked",
                dangerous_cmd
            );

            if let Err(MongoCoreError::ValidationError(msg)) = result {
                assert!(
                    msg.contains(dangerous_cmd),
                    "Error message should contain command name '{}', got: {}",
                    dangerous_cmd,
                    msg
                );
            } else {
                panic!("Expected ValidationError for command '{}'", dangerous_cmd);
            }
        }
    }

    #[test]
    fn test_command_with_multiple_operations() {
        let mode = ValidationMode::BlockDangerous;

        // Command with multiple keys, including a dangerous one
        let cmd = doc! {
            "find": "users",
            "dropDatabase": 1
        };

        let result = validate_command(&cmd, &mode);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_command() {
        let mode = ValidationMode::BlockDangerous;
        let cmd = doc! {};

        // Empty command should pass validation (will fail during execution)
        assert!(validate_command(&cmd, &mode).is_ok());
    }

    #[test]
    fn test_case_sensitivity() {
        let mode = ValidationMode::BlockDangerous;

        // MongoDB commands are case-sensitive, so different casing should be allowed
        let cmd = doc! { "DropDatabase": 1 };
        assert!(validate_command(&cmd, &mode).is_ok());

        let cmd = doc! { "DROPDATABASE": 1 };
        assert!(validate_command(&cmd, &mode).is_ok());

        // Exact match should be blocked
        let cmd = doc! { "dropDatabase": 1 };
        assert!(validate_command(&cmd, &mode).is_err());
    }
}

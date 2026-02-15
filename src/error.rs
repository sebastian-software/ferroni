// error.rs - Idiomatic Rust error types for Ferroni.
//
// Groups the ~100 C-style i32 error codes into semantic variants
// while preserving the original code for interop.

use std::fmt;

use crate::oniguruma::*;
use crate::regerror::onig_error_code_to_format;

/// Error type for regex compilation and matching operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegexError {
    /// Memory allocation failure.
    Memory,
    /// Match-stack limit exceeded.
    MatchStackLimitOver,
    /// Retry limit in match exceeded.
    RetryLimitInMatchOver,
    /// Retry limit in search exceeded.
    RetryLimitInSearchOver,
    /// Subexpression call limit exceeded.
    SubexpCallLimitOver,
    /// Time limit exceeded.
    TimeLimitOver,
    /// Parse depth limit exceeded.
    ParseDepthLimitOver,
    /// Syntax error in the pattern.
    Syntax { code: i32, message: String },
    /// Invalid argument passed to a function.
    InvalidArgument,
    /// Internal engine bug (should not occur in correct usage).
    InternalBug { code: i32, message: String },
    /// Library not initialized.
    NotInitialized,
    /// Invalid encoding or encoding combination.
    Encoding { code: i32, message: String },
    /// Other error not covered by specific variants.
    Other(i32),
}

impl fmt::Display for RegexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegexError::Memory => write!(f, "memory allocation failed"),
            RegexError::MatchStackLimitOver => write!(f, "match-stack limit over"),
            RegexError::RetryLimitInMatchOver => write!(f, "retry-limit-in-match over"),
            RegexError::RetryLimitInSearchOver => write!(f, "retry-limit-in-search over"),
            RegexError::SubexpCallLimitOver => write!(f, "subexp-call-limit-in-search over"),
            RegexError::TimeLimitOver => write!(f, "time limit over"),
            RegexError::ParseDepthLimitOver => write!(f, "parse depth limit over"),
            RegexError::Syntax { message, .. } => write!(f, "syntax error: {}", message),
            RegexError::InvalidArgument => write!(f, "invalid argument"),
            RegexError::InternalBug { message, .. } => write!(f, "internal error: {}", message),
            RegexError::NotInitialized => write!(f, "library is not initialized"),
            RegexError::Encoding { message, .. } => write!(f, "encoding error: {}", message),
            RegexError::Other(code) => write!(f, "error code {}", code),
        }
    }
}

impl std::error::Error for RegexError {}

impl From<i32> for RegexError {
    fn from(code: i32) -> Self {
        match code {
            ONIGERR_MEMORY => RegexError::Memory,
            ONIGERR_MATCH_STACK_LIMIT_OVER => RegexError::MatchStackLimitOver,
            ONIGERR_RETRY_LIMIT_IN_MATCH_OVER => RegexError::RetryLimitInMatchOver,
            ONIGERR_RETRY_LIMIT_IN_SEARCH_OVER => RegexError::RetryLimitInSearchOver,
            ONIGERR_SUBEXP_CALL_LIMIT_IN_SEARCH_OVER => RegexError::SubexpCallLimitOver,
            ONIGERR_TIME_LIMIT_OVER => RegexError::TimeLimitOver,
            ONIGERR_PARSE_DEPTH_LIMIT_OVER => RegexError::ParseDepthLimitOver,
            ONIGERR_INVALID_ARGUMENT => RegexError::InvalidArgument,
            ONIGERR_LIBRARY_IS_NOT_INITIALIZED => RegexError::NotInitialized,

            // Internal bugs
            ONIGERR_TYPE_BUG | ONIGERR_PARSER_BUG | ONIGERR_STACK_BUG
            | ONIGERR_UNDEFINED_BYTECODE | ONIGERR_UNEXPECTED_BYTECODE => {
                RegexError::InternalBug {
                    code,
                    message: onig_error_code_to_format(code).to_string(),
                }
            }

            // Encoding errors
            ONIGERR_DEFAULT_ENCODING_IS_NOT_SET
            | ONIGERR_SPECIFIED_ENCODING_CANT_CONVERT_TO_WIDE_CHAR
            | ONIGERR_NOT_SUPPORTED_ENCODING_COMBINATION => RegexError::Encoding {
                code,
                message: onig_error_code_to_format(code).to_string(),
            },

            // Syntax / pattern errors (range -100..-999)
            c if onig_is_pattern_error(c) => RegexError::Syntax {
                code: c,
                message: onig_error_code_to_format(c).to_string(),
            },

            _ => RegexError::Other(code),
        }
    }
}

impl RegexError {
    /// Returns the original C error code, if applicable.
    pub fn code(&self) -> i32 {
        match self {
            RegexError::Memory => ONIGERR_MEMORY,
            RegexError::MatchStackLimitOver => ONIGERR_MATCH_STACK_LIMIT_OVER,
            RegexError::RetryLimitInMatchOver => ONIGERR_RETRY_LIMIT_IN_MATCH_OVER,
            RegexError::RetryLimitInSearchOver => ONIGERR_RETRY_LIMIT_IN_SEARCH_OVER,
            RegexError::SubexpCallLimitOver => ONIGERR_SUBEXP_CALL_LIMIT_IN_SEARCH_OVER,
            RegexError::TimeLimitOver => ONIGERR_TIME_LIMIT_OVER,
            RegexError::ParseDepthLimitOver => ONIGERR_PARSE_DEPTH_LIMIT_OVER,
            RegexError::InvalidArgument => ONIGERR_INVALID_ARGUMENT,
            RegexError::NotInitialized => ONIGERR_LIBRARY_IS_NOT_INITIALIZED,
            RegexError::Syntax { code, .. } => *code,
            RegexError::InternalBug { code, .. } => *code,
            RegexError::Encoding { code, .. } => *code,
            RegexError::Other(code) => *code,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_memory_error() {
        let err = RegexError::from(ONIGERR_MEMORY);
        assert!(matches!(err, RegexError::Memory));
        assert_eq!(err.code(), ONIGERR_MEMORY);
        assert_eq!(err.to_string(), "memory allocation failed");
    }

    #[test]
    fn from_syntax_error() {
        let err = RegexError::from(ONIGERR_PREMATURE_END_OF_CHAR_CLASS);
        assert!(matches!(err, RegexError::Syntax { .. }));
        assert_eq!(err.code(), ONIGERR_PREMATURE_END_OF_CHAR_CLASS);
        assert!(err.to_string().contains("syntax error"));
    }

    #[test]
    fn from_internal_bug() {
        let err = RegexError::from(ONIGERR_PARSER_BUG);
        assert!(matches!(err, RegexError::InternalBug { .. }));
    }

    #[test]
    fn from_encoding_error() {
        let err = RegexError::from(ONIGERR_NOT_SUPPORTED_ENCODING_COMBINATION);
        assert!(matches!(err, RegexError::Encoding { .. }));
    }

    #[test]
    fn from_unknown_code() {
        let err = RegexError::from(-9999);
        assert!(matches!(err, RegexError::Other(-9999)));
    }

    #[test]
    fn display_impl() {
        let err = RegexError::InvalidArgument;
        assert_eq!(format!("{}", err), "invalid argument");
    }

    #[test]
    fn error_trait() {
        let err: Box<dyn std::error::Error> = Box::new(RegexError::Memory);
        assert_eq!(err.to_string(), "memory allocation failed");
    }
}

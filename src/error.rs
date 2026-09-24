// error.rs - Idiomatic Rust error types for Ferroni.
//
// Groups the ~100 C-style i32 error codes into semantic variants
// while preserving the original code for interop.

use std::fmt;

use crate::oniguruma::*;
use crate::regerror::{onig_error_code_to_str, onig_error_code_to_str_without_param};

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
    /// Builds the error from a bare code. A message that would name a group
    /// or property drops the name placeholder, as there is no name to show.
    fn from(code: i32) -> Self {
        RegexError::with_message(code, onig_error_code_to_str_without_param)
    }
}

impl RegexError {
    /// Builds the error `onig_new` reports. With the name the parser recorded
    /// (C's `einfo->par`), the message is the one C's
    /// `onig_error_code_to_str(s, code, einfo)` prints. Without one, where C
    /// would print an empty `<>`, the placeholder is dropped as in
    /// `RegexError::from(code)`.
    pub(crate) fn from_error_name(code: i32, name: Option<&[u8]>) -> Self {
        match name {
            Some(name) => RegexError::with_message(code, |c| onig_error_code_to_str(c, Some(name))),
            None => RegexError::from(code),
        }
    }

    fn with_message(code: i32, message: impl FnOnce(i32) -> String) -> Self {
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
            ONIGERR_TYPE_BUG
            | ONIGERR_PARSER_BUG
            | ONIGERR_STACK_BUG
            | ONIGERR_UNDEFINED_BYTECODE
            | ONIGERR_UNEXPECTED_BYTECODE => RegexError::InternalBug {
                code,
                message: message(code),
            },

            // Encoding errors
            ONIGERR_DEFAULT_ENCODING_IS_NOT_SET
            | ONIGERR_SPECIFIED_ENCODING_CANT_CONVERT_TO_WIDE_CHAR
            | ONIGERR_NOT_SUPPORTED_ENCODING_COMBINATION => RegexError::Encoding {
                code,
                message: message(code),
            },

            // Syntax / pattern errors (range -100..-999)
            c if onig_is_pattern_error(c) => RegexError::Syntax {
                code: c,
                message: message(c),
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
    fn from_code_drops_name_placeholder() {
        let cases = [
            (ONIGERR_INVALID_GROUP_NAME, "invalid group name"),
            (
                ONIGERR_INVALID_CHAR_IN_GROUP_NAME,
                "invalid char in group name",
            ),
            (ONIGERR_UNDEFINED_NAME_REFERENCE, "undefined name reference"),
            (
                ONIGERR_UNDEFINED_GROUP_REFERENCE,
                "undefined group reference",
            ),
            (ONIGERR_MULTIPLEX_DEFINED_NAME, "multiplex defined name"),
            (
                ONIGERR_MULTIPLEX_DEFINITION_NAME_CALL,
                "multiplex definition name call",
            ),
            (
                ONIGERR_INVALID_CHAR_PROPERTY_NAME,
                "invalid character property name",
            ),
        ];
        for (code, message) in cases {
            assert_eq!(
                RegexError::from(code),
                RegexError::Syntax {
                    code,
                    message: message.to_string()
                }
            );
        }
    }

    #[test]
    fn from_error_name_substitutes_name() {
        let code = ONIGERR_INVALID_CHAR_PROPERTY_NAME;
        let err = RegexError::from_error_name(code, Some(b"Nope"));
        assert_eq!(
            err.to_string(),
            "syntax error: invalid character property name {Nope}"
        );
        // A recorded empty name stays, as in C.
        let err = RegexError::from_error_name(code, Some(b""));
        assert_eq!(
            err.to_string(),
            "syntax error: invalid character property name {}"
        );
        // No recorded name: the placeholder goes, where C prints `{}`.
        let err = RegexError::from_error_name(code, None);
        assert_eq!(
            err.to_string(),
            "syntax error: invalid character property name"
        );
        // Codes without a name ignore it.
        let err = RegexError::from_error_name(ONIGERR_MEMORY, Some(b"Nope"));
        assert_eq!(err, RegexError::Memory);
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

    #[test]
    fn display_all_variants() {
        let cases: Vec<(RegexError, &str)> = vec![
            (RegexError::Memory, "memory allocation failed"),
            (RegexError::MatchStackLimitOver, "match-stack limit over"),
            (
                RegexError::RetryLimitInMatchOver,
                "retry-limit-in-match over",
            ),
            (
                RegexError::RetryLimitInSearchOver,
                "retry-limit-in-search over",
            ),
            (
                RegexError::SubexpCallLimitOver,
                "subexp-call-limit-in-search over",
            ),
            (RegexError::TimeLimitOver, "time limit over"),
            (RegexError::ParseDepthLimitOver, "parse depth limit over"),
            (
                RegexError::Syntax {
                    code: -100,
                    message: "bad pattern".into(),
                },
                "syntax error: bad pattern",
            ),
            (RegexError::InvalidArgument, "invalid argument"),
            (
                RegexError::InternalBug {
                    code: -1,
                    message: "oops".into(),
                },
                "internal error: oops",
            ),
            (RegexError::NotInitialized, "library is not initialized"),
            (
                RegexError::Encoding {
                    code: -21,
                    message: "bad enc".into(),
                },
                "encoding error: bad enc",
            ),
            (RegexError::Other(-9999), "error code -9999"),
        ];
        for (err, expected) in cases {
            assert_eq!(err.to_string(), expected);
        }
    }

    #[test]
    fn code_all_variants() {
        assert_eq!(RegexError::Memory.code(), ONIGERR_MEMORY);
        assert_eq!(
            RegexError::MatchStackLimitOver.code(),
            ONIGERR_MATCH_STACK_LIMIT_OVER
        );
        assert_eq!(
            RegexError::RetryLimitInMatchOver.code(),
            ONIGERR_RETRY_LIMIT_IN_MATCH_OVER
        );
        assert_eq!(
            RegexError::RetryLimitInSearchOver.code(),
            ONIGERR_RETRY_LIMIT_IN_SEARCH_OVER
        );
        assert_eq!(
            RegexError::SubexpCallLimitOver.code(),
            ONIGERR_SUBEXP_CALL_LIMIT_IN_SEARCH_OVER
        );
        assert_eq!(RegexError::TimeLimitOver.code(), ONIGERR_TIME_LIMIT_OVER);
        assert_eq!(
            RegexError::ParseDepthLimitOver.code(),
            ONIGERR_PARSE_DEPTH_LIMIT_OVER
        );
        assert_eq!(RegexError::InvalidArgument.code(), ONIGERR_INVALID_ARGUMENT);
        assert_eq!(
            RegexError::NotInitialized.code(),
            ONIGERR_LIBRARY_IS_NOT_INITIALIZED
        );
        assert_eq!(
            RegexError::Syntax {
                code: -100,
                message: String::new()
            }
            .code(),
            -100
        );
        assert_eq!(
            RegexError::InternalBug {
                code: -5,
                message: String::new()
            }
            .code(),
            -5
        );
        assert_eq!(
            RegexError::Encoding {
                code: -21,
                message: String::new()
            }
            .code(),
            -21
        );
        assert_eq!(RegexError::Other(-9999).code(), -9999);
    }

    #[test]
    fn from_limit_errors() {
        assert!(matches!(
            RegexError::from(ONIGERR_MATCH_STACK_LIMIT_OVER),
            RegexError::MatchStackLimitOver
        ));
        assert!(matches!(
            RegexError::from(ONIGERR_RETRY_LIMIT_IN_MATCH_OVER),
            RegexError::RetryLimitInMatchOver
        ));
        assert!(matches!(
            RegexError::from(ONIGERR_RETRY_LIMIT_IN_SEARCH_OVER),
            RegexError::RetryLimitInSearchOver
        ));
        assert!(matches!(
            RegexError::from(ONIGERR_SUBEXP_CALL_LIMIT_IN_SEARCH_OVER),
            RegexError::SubexpCallLimitOver
        ));
        assert!(matches!(
            RegexError::from(ONIGERR_TIME_LIMIT_OVER),
            RegexError::TimeLimitOver
        ));
        assert!(matches!(
            RegexError::from(ONIGERR_PARSE_DEPTH_LIMIT_OVER),
            RegexError::ParseDepthLimitOver
        ));
        assert!(matches!(
            RegexError::from(ONIGERR_INVALID_ARGUMENT),
            RegexError::InvalidArgument
        ));
        assert!(matches!(
            RegexError::from(ONIGERR_LIBRARY_IS_NOT_INITIALIZED),
            RegexError::NotInitialized
        ));
    }
}

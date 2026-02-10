// regerror.rs - Port of regerror.c
// Error code to string conversion.
//
// This is a 1:1 port of oniguruma's regerror.c.
// Maps error codes to human-readable messages.

use crate::oniguruma::*;

/// Get the format string for an error code.
/// Corresponds to C's onig_error_code_to_format().
pub fn onig_error_code_to_format(code: i32) -> &'static str {
    match code {
        ONIG_MISMATCH => "mismatch",
        ONIG_NO_SUPPORT_CONFIG => "no support in this configuration",
        ONIG_ABORT => "abort",
        ONIGERR_MEMORY => "fail to memory allocation",
        ONIGERR_MATCH_STACK_LIMIT_OVER => "match-stack limit over",
        ONIGERR_PARSE_DEPTH_LIMIT_OVER => "parse depth limit over",
        ONIGERR_RETRY_LIMIT_IN_MATCH_OVER => "retry-limit-in-match over",
        ONIGERR_RETRY_LIMIT_IN_SEARCH_OVER => "retry-limit-in-search over",
        ONIGERR_SUBEXP_CALL_LIMIT_IN_SEARCH_OVER => "subexp-call-limit-in-search over",
        ONIGERR_TYPE_BUG => "undefined type (bug)",
        ONIGERR_PARSER_BUG => "internal parser error (bug)",
        ONIGERR_STACK_BUG => "stack error (bug)",
        ONIGERR_UNDEFINED_BYTECODE => "undefined bytecode (bug)",
        ONIGERR_UNEXPECTED_BYTECODE => "unexpected bytecode (bug)",
        ONIGERR_DEFAULT_ENCODING_IS_NOT_SET => "default multibyte-encoding is not set",
        ONIGERR_SPECIFIED_ENCODING_CANT_CONVERT_TO_WIDE_CHAR => {
            "can't convert to wide-char on specified multibyte-encoding"
        }
        ONIGERR_FAIL_TO_INITIALIZE => "fail to initialize",
        ONIGERR_INVALID_ARGUMENT => "invalid argument",
        ONIGERR_END_PATTERN_AT_LEFT_BRACE => "end pattern at left brace",
        ONIGERR_END_PATTERN_AT_LEFT_BRACKET => "end pattern at left bracket",
        ONIGERR_EMPTY_CHAR_CLASS => "empty char-class",
        ONIGERR_PREMATURE_END_OF_CHAR_CLASS => "premature end of char-class",
        ONIGERR_END_PATTERN_AT_ESCAPE => "end pattern at escape",
        ONIGERR_END_PATTERN_AT_META => "end pattern at meta",
        ONIGERR_END_PATTERN_AT_CONTROL => "end pattern at control",
        ONIGERR_META_CODE_SYNTAX => "invalid meta-code syntax",
        ONIGERR_CONTROL_CODE_SYNTAX => "invalid control-code syntax",
        ONIGERR_CHAR_CLASS_VALUE_AT_END_OF_RANGE => "char-class value at end of range",
        ONIGERR_CHAR_CLASS_VALUE_AT_START_OF_RANGE => "char-class value at start of range",
        ONIGERR_UNMATCHED_RANGE_SPECIFIER_IN_CHAR_CLASS => {
            "unmatched range specifier in char-class"
        }
        ONIGERR_TARGET_OF_REPEAT_OPERATOR_NOT_SPECIFIED => {
            "target of repeat operator is not specified"
        }
        ONIGERR_TARGET_OF_REPEAT_OPERATOR_INVALID => "target of repeat operator is invalid",
        ONIGERR_NESTED_REPEAT_OPERATOR => "nested repeat operator",
        ONIGERR_UNMATCHED_CLOSE_PARENTHESIS => "unmatched close parenthesis",
        ONIGERR_END_PATTERN_WITH_UNMATCHED_PARENTHESIS => {
            "end pattern with unmatched parenthesis"
        }
        ONIGERR_END_PATTERN_IN_GROUP => "end pattern in group",
        ONIGERR_UNDEFINED_GROUP_OPTION => "undefined group option",
        ONIGERR_INVALID_GROUP_OPTION => "invalid group option",
        ONIGERR_INVALID_POSIX_BRACKET_TYPE => "invalid POSIX bracket type",
        ONIGERR_INVALID_LOOK_BEHIND_PATTERN => "invalid pattern in look-behind",
        ONIGERR_INVALID_REPEAT_RANGE_PATTERN => "invalid repeat range {lower,upper}",
        ONIGERR_TOO_BIG_NUMBER => "too big number",
        ONIGERR_TOO_BIG_NUMBER_FOR_REPEAT_RANGE => "too big number for repeat range",
        ONIGERR_UPPER_SMALLER_THAN_LOWER_IN_REPEAT_RANGE => {
            "upper is smaller than lower in repeat range"
        }
        ONIGERR_EMPTY_RANGE_IN_CHAR_CLASS => "empty range in char class",
        ONIGERR_MISMATCH_CODE_LENGTH_IN_CLASS_RANGE => {
            "mismatch multibyte code length in char-class range"
        }
        ONIGERR_TOO_MANY_MULTI_BYTE_RANGES => "too many multibyte code ranges are specified",
        ONIGERR_TOO_SHORT_MULTI_BYTE_STRING => "too short multibyte code string",
        ONIGERR_TOO_BIG_BACKREF_NUMBER => "too big backref number",
        ONIGERR_INVALID_BACKREF => "invalid backref number/name",
        ONIGERR_NUMBERED_BACKREF_OR_CALL_NOT_ALLOWED => {
            "numbered backref/call is not allowed. (use name)"
        }
        ONIGERR_TOO_MANY_CAPTURES => "too many captures",
        ONIGERR_TOO_BIG_WIDE_CHAR_VALUE => "too big wide-char value",
        ONIGERR_TOO_LONG_WIDE_CHAR_VALUE => "too long wide-char value",
        ONIGERR_UNDEFINED_OPERATOR => "undefined operator",
        ONIGERR_INVALID_CODE_POINT_VALUE => "invalid code point value",
        ONIGERR_EMPTY_GROUP_NAME => "group name is empty",
        ONIGERR_INVALID_GROUP_NAME => "invalid group name <%n>",
        ONIGERR_INVALID_CHAR_IN_GROUP_NAME => "invalid char in group name <%n>",
        ONIGERR_UNDEFINED_NAME_REFERENCE => "undefined name <%n> reference",
        ONIGERR_UNDEFINED_GROUP_REFERENCE => "undefined group <%n> reference",
        ONIGERR_MULTIPLEX_DEFINED_NAME => "multiplex defined name <%n>",
        ONIGERR_MULTIPLEX_DEFINITION_NAME_CALL => "multiplex definition name <%n> call",
        ONIGERR_NEVER_ENDING_RECURSION => "never ending recursion",
        ONIGERR_GROUP_NUMBER_OVER_FOR_CAPTURE_HISTORY => {
            "group number is too big for capture history"
        }
        ONIGERR_INVALID_CHAR_PROPERTY_NAME => "invalid character property name {%n}",
        ONIGERR_INVALID_IF_ELSE_SYNTAX => "invalid if-else syntax",
        ONIGERR_INVALID_ABSENT_GROUP_PATTERN => "invalid absent group pattern",
        ONIGERR_INVALID_ABSENT_GROUP_GENERATOR_PATTERN => {
            "invalid absent group generator pattern"
        }
        ONIGERR_INVALID_CALLOUT_PATTERN => "invalid callout pattern",
        ONIGERR_INVALID_CALLOUT_NAME => "invalid callout name",
        ONIGERR_UNDEFINED_CALLOUT_NAME => "undefined callout name",
        ONIGERR_INVALID_CALLOUT_BODY => "invalid callout body",
        ONIGERR_INVALID_CALLOUT_TAG_NAME => "invalid callout tag name",
        ONIGERR_INVALID_CALLOUT_ARG => "invalid callout arg",
        ONIGERR_NOT_SUPPORTED_ENCODING_COMBINATION => "not supported encoding combination",
        ONIGERR_INVALID_COMBINATION_OF_OPTIONS => "invalid combination of options",
        ONIGERR_VERY_INEFFICIENT_PATTERN => "very inefficient pattern",
        ONIGERR_LIBRARY_IS_NOT_INITIALIZED => "library is not initialized",
        _ => "undefined error code",
    }
}

/// Check if an error code requires a parameter (name/pattern text).
pub fn onig_is_error_code_needs_param(code: i32) -> bool {
    matches!(
        code,
        ONIGERR_UNDEFINED_NAME_REFERENCE
            | ONIGERR_UNDEFINED_GROUP_REFERENCE
            | ONIGERR_MULTIPLEX_DEFINED_NAME
            | ONIGERR_MULTIPLEX_DEFINITION_NAME_CALL
            | ONIGERR_INVALID_GROUP_NAME
            | ONIGERR_INVALID_CHAR_IN_GROUP_NAME
            | ONIGERR_INVALID_CHAR_PROPERTY_NAME
    )
}

/// Convert an error code to a human-readable string.
/// For parameterized errors, pass the parameter text in `param`.
/// Corresponds to C's onig_error_code_to_str().
pub fn onig_error_code_to_str(code: i32, param: Option<&[u8]>) -> String {
    let fmt = onig_error_code_to_format(code);

    if onig_is_error_code_needs_param(code) {
        if let Some(par) = param {
            // Replace %n with the parameter text (converted to ASCII-safe)
            let par_str = par
                .iter()
                .map(|&b| {
                    if b.is_ascii_graphic() || b == b' ' {
                        (b as char).to_string()
                    } else {
                        format!("\\x{:02x}", b)
                    }
                })
                .collect::<String>();
            fmt.replace("%n", &par_str)
        } else {
            fmt.replace("%n", "")
        }
    } else {
        fmt.to_string()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mismatch() {
        assert_eq!(onig_error_code_to_str(ONIG_MISMATCH, None), "mismatch");
    }

    #[test]
    fn test_memory() {
        assert_eq!(
            onig_error_code_to_str(ONIGERR_MEMORY, None),
            "fail to memory allocation"
        );
    }

    #[test]
    fn test_undefined_error() {
        assert_eq!(
            onig_error_code_to_str(-9999, None),
            "undefined error code"
        );
    }

    #[test]
    fn test_parameterized_error() {
        let msg = onig_error_code_to_str(
            ONIGERR_UNDEFINED_NAME_REFERENCE,
            Some(b"foo"),
        );
        assert_eq!(msg, "undefined name <foo> reference");
    }

    #[test]
    fn test_parameterized_error_no_param() {
        let msg = onig_error_code_to_str(ONIGERR_UNDEFINED_NAME_REFERENCE, None);
        assert_eq!(msg, "undefined name <> reference");
    }

    #[test]
    fn test_needs_param() {
        assert!(onig_is_error_code_needs_param(ONIGERR_UNDEFINED_NAME_REFERENCE));
        assert!(!onig_is_error_code_needs_param(ONIGERR_MEMORY));
        assert!(!onig_is_error_code_needs_param(ONIG_MISMATCH));
    }
}

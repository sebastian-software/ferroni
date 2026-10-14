/* Compile capture-history patterns with a per-regex syntax copy. */
#include <stddef.h>
#include "oniguruma.h"

int ferroni_onig_new_capture_history(regex_t **regex,
                                    const unsigned char *pattern,
                                    size_t length,
                                    OnigOptionType options,
                                    OnigErrorInfo *error_info) {
  OnigSyntaxType syntax = OnigSyntaxOniguruma;
  syntax.op2 |= ONIG_SYN_OP2_ATMARK_CAPTURE_HISTORY;
  return onig_new(regex, pattern, pattern + length, options, ONIG_ENCODING_UTF8,
                  &syntax, error_info);
}

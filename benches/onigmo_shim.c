/* Bridge from the benchmarks to Ruby's Onigmo, built by build.rs under the
 * `onigmo` feature. Every Onigmo symbol is renamed with a prefix there, so
 * Onigmo links next to upstream Oniguruma; these entry points keep the Rust
 * side free of Onigmo's struct layouts. */
#include <stdlib.h>
#include "onigmo.h"

typedef struct {
  regex_t *reg;
  OnigRegion *region;
} ferroni_onigmo_regex;

int ferroni_onigmo_init(void) {
  OnigEncoding encodings[] = {ONIG_ENCODING_UTF8};
  return onig_initialize(encodings, 1);
}

/* Ruby syntax, which Oniguruma's own syntax descends from. */
ferroni_onigmo_regex *ferroni_onigmo_new(const unsigned char *pattern, size_t length,
                                         int ignorecase, int capture_group, int *error) {
  OnigErrorInfo info;
  OnigOptionType options = ONIG_OPTION_NONE;
  ferroni_onigmo_regex *regex = malloc(sizeof(*regex));
  if (ignorecase) options |= ONIG_OPTION_IGNORECASE;
  if (capture_group) options |= ONIG_OPTION_CAPTURE_GROUP;
  *error = onig_new(&regex->reg, pattern, pattern + length, options, ONIG_ENCODING_UTF8,
                    ONIG_SYNTAX_RUBY, &info);
  if (*error != ONIG_NORMAL) {
    free(regex);
    return NULL;
  }
  regex->region = onig_region_new();
  return regex;
}

void ferroni_onigmo_free(ferroni_onigmo_regex *regex) {
  onig_region_free(regex->region, 1);
  onig_free(regex->reg);
  free(regex);
}

/* The match start, ONIG_MISMATCH, or an error code. With `want_region`, the
 * captures stay readable until the next search on this regex. */
long ferroni_onigmo_search(ferroni_onigmo_regex *regex, const unsigned char *text,
                           size_t length, size_t start, int want_region) {
  return (long)onig_search(regex->reg, text, text + length, text + start, text + length,
                           want_region ? regex->region : NULL, ONIG_OPTION_NONE);
}

int ferroni_onigmo_num_regs(const ferroni_onigmo_regex *regex) {
  return regex->region->num_regs;
}

long ferroni_onigmo_beg(const ferroni_onigmo_regex *regex, int group) {
  return (long)regex->region->beg[group];
}

long ferroni_onigmo_end(const ferroni_onigmo_regex *regex, int group) {
  return (long)regex->region->end[group];
}

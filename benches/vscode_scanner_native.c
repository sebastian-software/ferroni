/*
 * vscode_scanner_native.c
 *
 * Extracted from vscode-oniguruma/src/onig.cc (Microsoft, MIT License).
 * Core scanner structs and functions for native benchmarking against Ferroni.
 *
 * The original code lives inside an `extern "C" {}` block and is pure C.
 * We reproduce it here as a plain C file to avoid Emscripten/C++ dependencies.
 */

#include <stdlib.h>
#include <string.h>
#include "oniguruma.h"

/* ---- Structs ---- */

typedef struct OnigRegExp_ {
    unsigned char *strData;
    int strLength;
    regex_t *regex;
    OnigRegion *region;
    int hasGAnchor;
    int lastSearchStrCacheId;
    int lastSearchPosition;
    int lastSearchOnigOption;
    int lastSearchMatched;
} OnigRegExp;

typedef struct OnigScanner_ {
    OnigRegSet *rset;
    OnigRegExp **regexes;
    int count;
} OnigScanner;

/* ---- Helpers ---- */

static int lastOnigStatus = 0;
static OnigErrorInfo lastOnigErrorInfo;

#define MAX_REGIONS 1000

static int *encodeOnigRegion(OnigRegion *result, int index) {
    static int encodedResult[2 * (1 + MAX_REGIONS)];
    int i;
    if (result == NULL || result->num_regs > MAX_REGIONS) {
        return NULL;
    }
    encodedResult[0] = index;
    encodedResult[1] = result->num_regs;
    for (i = 0; i < result->num_regs; i++) {
        encodedResult[2 * i + 2] = result->beg[i];
        encodedResult[2 * i + 3] = result->end[i];
    }
    return encodedResult;
}

/* ---- OnigRegExp ---- */

static int hasGAnchor(unsigned char *str, int len) {
    int pos;
    for (pos = 0; pos < len; pos++) {
        if (str[pos] == '\\' && pos + 1 < len) {
            if (str[pos + 1] == 'G') {
                return 1;
            }
        }
    }
    return 0;
}

static OnigRegExp *createOnigRegExp(unsigned char *data, int length, int options,
                                     OnigSyntaxType *syntax) {
    OnigRegExp *result;
    regex_t *regex;

    lastOnigStatus =
        onig_new(&regex, data, data + length, options, ONIG_ENCODING_UTF8,
                 syntax, &lastOnigErrorInfo);

    if (lastOnigStatus != ONIG_NORMAL) {
        return NULL;
    }

    result = (OnigRegExp *)malloc(sizeof(OnigRegExp));
    result->strLength = length;
    result->strData = (unsigned char *)malloc(length);
    memcpy(result->strData, data, length);
    result->regex = regex;
    result->region = onig_region_new();
    result->hasGAnchor = hasGAnchor(data, length);
    result->lastSearchStrCacheId = 0;
    result->lastSearchPosition = 0;
    result->lastSearchOnigOption = ONIG_OPTION_NONE;
    result->lastSearchMatched = 0;
    return result;
}

static void freeOnigRegExp(OnigRegExp *regex) {
    free(regex->strData);
    onig_region_free(regex->region, 1);
    free(regex);
}

static OnigRegion *_searchOnigRegExp(OnigRegExp *regex, unsigned char *strData,
                                      int strLength, int position,
                                      OnigOptionType onigOption) {
    int status;
    status = onig_search(regex->regex, strData, strData + strLength,
                         strData + position, strData + strLength, regex->region,
                         onigOption);
    if (status == ONIG_MISMATCH || status < 0) {
        regex->lastSearchMatched = 0;
        return NULL;
    }
    regex->lastSearchMatched = 1;
    return regex->region;
}

static OnigRegion *searchOnigRegExp(OnigRegExp *regex, int strCacheId,
                                     unsigned char *strData, int strLength,
                                     int position, OnigOptionType onigOption) {
    if (regex->hasGAnchor) {
        return _searchOnigRegExp(regex, strData, strLength, position, onigOption);
    }
    if (regex->lastSearchStrCacheId == strCacheId &&
        regex->lastSearchOnigOption == (int)onigOption &&
        regex->lastSearchPosition <= position) {
        if (!regex->lastSearchMatched) {
            return NULL;
        }
        if (regex->region->beg[0] >= position) {
            return regex->region;
        }
    }
    regex->lastSearchStrCacheId = strCacheId;
    regex->lastSearchPosition = position;
    regex->lastSearchOnigOption = (int)onigOption;
    return _searchOnigRegExp(regex, strData, strLength, position, onigOption);
}

/* ---- OnigScanner (public API) ---- */

OnigScanner *createOnigScanner(unsigned char **patterns, int *lengths, int count,
                               int options, OnigSyntaxType *syntax) {
    int i, j;
    OnigRegExp **regexes;
    regex_t **regs;
    OnigRegSet *rset;
    OnigScanner *scanner;

    regexes = (OnigRegExp **)malloc(sizeof(OnigRegExp *) * count);
    regs = (regex_t **)malloc(sizeof(regex_t *) * count);

    for (i = 0; i < count; i++) {
        regexes[i] = createOnigRegExp(patterns[i], lengths[i], options, syntax);
        if (regexes[i] != NULL) {
            regs[i] = regexes[i]->regex;
        } else {
            for (j = 0; j < i; j++) {
                free(regs[j]);
                freeOnigRegExp(regexes[j]);
            }
            free(regexes);
            free(regs);
            return NULL;
        }
    }

    onig_regset_new(&rset, count, regs);
    free(regs);

    scanner = (OnigScanner *)malloc(sizeof(OnigScanner));
    scanner->rset = rset;
    scanner->regexes = regexes;
    scanner->count = count;
    return scanner;
}

void freeOnigScanner(OnigScanner *scanner) {
    int i;
    for (i = 0; i < scanner->count; i++) {
        freeOnigRegExp(scanner->regexes[i]);
    }
    free(scanner->regexes);
    onig_regset_free(scanner->rset);
    free(scanner);
}

int *findNextOnigScannerMatch(OnigScanner *scanner, int strCacheId,
                               unsigned char *strData, int strLength,
                               int position, int options) {
    int bestLocation = 0;
    int bestResultIndex = 0;
    OnigRegion *bestResult = NULL;
    OnigRegion *result;
    int i;
    int location;

    if (strLength < 1000) {
        bestResultIndex = onig_regset_search(
            scanner->rset, strData, strData + strLength, strData + position,
            strData + strLength, ONIG_REGSET_POSITION_LEAD, options,
            &bestLocation);
        if (bestResultIndex < 0) {
            return NULL;
        }
        return encodeOnigRegion(
            onig_regset_get_region(scanner->rset, bestResultIndex),
            bestResultIndex);
    }

    for (i = 0; i < scanner->count; i++) {
        result = searchOnigRegExp(scanner->regexes[i], strCacheId, strData,
                                  strLength, position, options);
        if (result != NULL && result->num_regs > 0) {
            location = result->beg[0];
            if (bestResult == NULL || location < bestLocation) {
                bestLocation = location;
                bestResult = result;
                bestResultIndex = i;
            }
            if (location == position) {
                break;
            }
        }
    }

    if (bestResult == NULL) {
        return NULL;
    }

    return encodeOnigRegion(bestResult, bestResultIndex);
}

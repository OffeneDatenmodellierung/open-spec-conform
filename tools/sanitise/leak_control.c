/*
 * A C program that leaks a string `conform-ffi` gave it, on purpose.
 *
 * The second half of the honesty check in `run.sh`. `negative_control.c`
 * proves the checker sees *invalid* accesses; this one asks a different
 * question, because a checker can see those and still not count allocations
 * that were never given back — and "no leaks" from a tool that is not looking
 * for leaks is the most comfortable false claim in this file's subject area.
 *
 * `run.sh` treats the answer differently in its two arms, and says which it
 * got:
 *
 *   - under **valgrind**, this program MUST be reported. Valgrind's leak
 *     check is not optional and a clean run here would mean the invocation is
 *     wrong.
 *   - under **AddressSanitizer**, it *should* be, but LeakSanitizer is not
 *     available on every platform — notably not on aarch64-apple-darwin,
 *     where this program runs to completion with `detect_leaks=1` set and
 *     nothing is said. `run.sh` reports that as a measured fact about the
 *     host rather than passing over it, because a reader deserves to know
 *     which half of "ASan clean" they actually got.
 *
 * SPDX-License-Identifier: MIT OR Apache-2.0
 */

#include <stdio.h>
#include <string.h>

#include "conform.h"

int main(int argc, char **argv)
{
    if (argc != 2) {
        fprintf(stderr, "usage: %s <specs.toml>\n", argv[0]);
        return 2;
    }

    ConformValidator *validator = conform_validator_new("odcs", argv[1]);
    if (validator == NULL) {
        fprintf(stderr, "could not build a validator: %s\n", conform_last_error());
        return 2;
    }

    const char *document = "version: 1.0.0\n";
    char *json = NULL;
    size_t json_len = 0;
    if (conform_validate(validator, "control.yaml", (const uint8_t *)document, strlen(document),
                         &json, &json_len)
        != CONFORM_STATUS_OK) {
        fprintf(stderr, "validation failed: %s\n", conform_last_error());
        conform_validator_free(validator);
        return 2;
    }

    /* `conform_string_free(json)` belongs here, and is deliberately absent. */
    printf("leaked %zu bytes on purpose\n", json_len);

    conform_validator_free(validator);
    return 0;
}

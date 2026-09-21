/*
 * A C program that abuses `conform-ffi` on purpose, so that a clean sanitiser
 * run means something.
 *
 * A sanitiser that is not actually running reports no errors, and so does a
 * library with no errors in it. The two are indistinguishable from the exit
 * code alone, which makes "ASan clean" the easiest claim in software to make
 * falsely — link it wrong, or forget the flag, and you get a green tick for
 * nothing.
 *
 * So `run.sh` builds this alongside the real smoke test and requires it to
 * *fail*. It frees a string the library handed it, and then frees it again.
 * The second free goes through the same interceptor as the first, which is
 * inside the sanitiser's allocator and does not care whether the caller was
 * instrumented — so this is detectable in exactly the configuration `run.sh`
 * builds, which is the point.
 *
 * If this program exits cleanly, the sanitiser is not watching, and `run.sh`
 * says so instead of reporting a pass.
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

    conform_string_free(json);
    conform_string_free(json); /* <- deliberate double free */

    conform_validator_free(validator);

    printf("the double free was not detected\n");
    return 0;
}

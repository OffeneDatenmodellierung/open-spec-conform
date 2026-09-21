/*
 * A C program that links `conform-ffi` and validates one document.
 *
 * This is the test that the header, the library and the ownership rules all
 * describe the same thing. Everything else in this crate's test suite is Rust
 * calling Rust through an `extern "C"` signature, which proves the logic but
 * not the *linkage*: it cannot catch a header that declares an argument the
 * library does not take, a symbol that was never exported, or a `char *` the
 * caller is told to free with the wrong function. A real C compiler and a real
 * linker catch all three.
 *
 * Driven by `tests/a_c_program_can_link_and_validate.rs`, which compiles this
 * file against the built library and runs it with:
 *
 *     conform_smoke <path to specs.toml> <path to a document>
 *
 * Exits 0 if every check passed, 1 otherwise, and says which check failed.
 *
 * SPDX-License-Identifier: MIT OR Apache-2.0
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "conform.h"

static int failures = 0;

static void check(int condition, const char *what)
{
    if (condition) {
        printf("ok   - %s\n", what);
    } else {
        printf("FAIL - %s\n", what);
        const char *why = conform_last_error();
        if (why != NULL) {
            printf("       last error: %s\n", why);
        }
        failures += 1;
    }
}

/* Read a whole file into a heap buffer. Returns NULL on failure. */
static unsigned char *slurp(const char *path, size_t *length)
{
    FILE *file = fopen(path, "rb");
    if (file == NULL) {
        return NULL;
    }
    if (fseek(file, 0, SEEK_END) != 0) {
        fclose(file);
        return NULL;
    }
    long size = ftell(file);
    if (size < 0) {
        fclose(file);
        return NULL;
    }
    rewind(file);

    unsigned char *bytes = malloc((size_t)size + 1);
    if (bytes == NULL) {
        fclose(file);
        return NULL;
    }
    size_t read = fread(bytes, 1, (size_t)size, file);
    fclose(file);
    if (read != (size_t)size) {
        free(bytes);
        return NULL;
    }
    bytes[size] = '\0';
    *length = read;
    return bytes;
}

int main(int argc, char **argv)
{
    if (argc != 3) {
        fprintf(stderr, "usage: %s <specs.toml> <document>\n", argv[0]);
        return 2;
    }
    const char *registry_path = argv[1];
    const char *document_path = argv[2];

    /* The library identifies itself, and the string is static: not freed. */
    const char *version = conform_version();
    check(version != NULL && version[0] != '\0', "conform_version returns a string");
    printf("       conform-ffi %s, ABI %u\n", version, (unsigned)CONFORM_ABI_VERSION);

    /*
     * The panic net, proved in the library this program actually linked. If
     * `conform-ffi` were built with `panic = "abort"` this call would take the
     * process down instead of returning, which is the diagnosis rather than a
     * test failure — and is exactly what a binding wants to find out at
     * start-up rather than in production.
     */
    check(conform_self_test_panic() == CONFORM_STATUS_PANIC,
          "a panic inside the library does not cross into C");

    /* Null in, refusal out — no dereference. */
    char *nothing = NULL;
    enum ConformStatus refused = conform_validate(NULL, "x", (const uint8_t *)"", 0, &nothing, NULL);
    check(refused == CONFORM_STATUS_NULL_POINTER, "a null handle is refused");
    check(nothing == NULL, "a refused call leaves nothing to free");

    /* An unknown spec id fails with an explanation rather than a handle. */
    check(conform_validator_new("no-such-standard", registry_path) == NULL,
          "an unknown spec id yields no handle");
    check(conform_last_error() != NULL, "and an explanation of why");

    /* The real thing. */
    ConformValidator *validator = conform_validator_new("odcs", registry_path);
    check(validator != NULL, "an odcs validator can be built from the registry");
    if (validator == NULL) {
        return 1;
    }

    size_t document_len = 0;
    unsigned char *document = slurp(document_path, &document_len);
    check(document != NULL && document_len > 0, "the document could be read");
    if (document == NULL) {
        conform_validator_free(validator);
        return 1;
    }

    char *json = NULL;
    size_t json_len = 0;
    enum ConformStatus status = conform_validate(validator, document_path, document, document_len,
                                                 &json, &json_len);
    check(status == CONFORM_STATUS_OK, "the document was validated");
    check(json != NULL, "and a report came back");
    if (json == NULL) {
        free(document);
        conform_validator_free(validator);
        return 1;
    }

    check(json_len == strlen(json), "the reported length matches the string");
    check(strstr(json, "\"schema_version\":1") != NULL, "the report carries its schema version");
    check(strstr(json, "\"spec\":{\"id\":\"odcs\"") != NULL, "and names the standard it used");
    check(strstr(json, "\"diagnostics\"") != NULL, "and carries a diagnostics array");

    /*
     * The fixture is a deliberately faulty contract, so a report with no
     * errors in it would mean the boundary handed back a verdict nobody
     * reached. `"error":0` appears in the document summary when there are
     * none; its absence there is what this looks for.
     */
    check(strstr(json, "\"error\":0,") == NULL,
          "a faulty document produced errors rather than silence");

    printf("       report is %zu bytes\n", json_len);

    /* Ownership: the string came from the library, so it goes back to it. */
    conform_string_free(json);
    conform_validator_free(validator);
    free(document);

    /* Both free functions accept null, so a cleanup path can be unconditional. */
    conform_string_free(NULL);
    conform_validator_free(NULL);
    check(1, "freeing null is accepted");

    if (failures != 0) {
        printf("%d check(s) failed\n", failures);
        return 1;
    }
    printf("all checks passed\n");
    return 0;
}

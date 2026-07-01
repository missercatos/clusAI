/*
 * C example for ai-ffi.
 *
 * Build: cc -o example example.c -L ../../target/release -lai_ffi
 * Run:   LD_LIBRARY_PATH=../../target/release ./example
 *
 * Or link statically against the header.
 */

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

/* Forward declarations (match the FFI exports) */
extern void* ai_init(const char* config_path);
extern int   ai_think(void* handle, const char* prompt);
extern int   ai_poll(void* handle, char* buf, int len);
extern int   ai_done(const void* handle);
extern int   ai_wait(void* handle, char* buf, int len);
extern void  ai_free(void* handle);
extern const char* ai_version(void);

int main(int argc, char** argv) {
    const char* config = (argc > 1) ? argv[1] : "./.clusai.toml";

    printf("ai-core version: %s\n", ai_version());

    void* h = ai_init(config);
    if (!h) {
        fprintf(stderr, "failed to init ai-core (check stderr for config errors)\n");
        return 1;
    }

    const char* prompts[] = {"Introduce yourself briefly.", NULL};
    for (int i = 0; prompts[i]; i++) {
        printf("\n>>> %s\n", prompts[i]);

        int rc = ai_think(h, prompts[i]);
        if (rc != 0) {
            fprintf(stderr, "ai_think failed\n");
            continue;
        }

        while (!ai_done(h)) {
            char buf[8192];
            int n = ai_poll(h, buf, sizeof(buf) - 1);
            if (n > 0) {
                buf[n] = '\0';
                printf("%s\n", buf);
            }
            usleep(10000); /* 10 ms */
        }
    }

    ai_free(h);
    return 0;
}

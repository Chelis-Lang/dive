#include <mimalloc.h>
#include <stdio.h>
#include <stdlib.h>
/* A single uncommitted arena reservation. No model graph or allocation loop. */
int main(int argc, char **argv) {
    if (argc != 2) return 2;
    unsigned long gib = strtoul(argv[1], NULL, 10);
    if (gib != 8 && gib != 16) return 2;
    int result = mi_reserve_os_memory((size_t)gib * 1024 * 1024 * 1024, false, false);
    printf("%d %lu %d\n", mi_version(), gib, result);
    return 0;
}

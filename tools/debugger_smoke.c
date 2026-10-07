/* Independently authored sacrificial x86 process for debugger lifecycle checks. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdio.h>
static volatile unsigned ticks;
__declspec(noinline) static void tick(void) { ++ticks; }
int main(int argc, char **argv) {
    (void)argv;
    printf("%08lx\n", (unsigned long)(uintptr_t)&tick);
    fflush(stdout);
    for (unsigned i = 0; i < 6000; ++i) {
        if (argc == 1) {
            __try { RaiseException(0xe06d7363, 0, 0, NULL); }
            __except(EXCEPTION_EXECUTE_HANDLER) { tick(); }
        } else { tick(); }
        Sleep(100);
    }
    return 0;
}

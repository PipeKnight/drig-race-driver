package org.reassembly.grid;

import static org.junit.Assert.assertEquals;

import org.junit.Test;

/** The log file names match the desktop run logs (player.rs setup_output). */
public class CrashLogTest {
    /** 2026-11-25 10:15:07 UTC. */
    private static final long TIME = 1_795_601_707_000L;

    @Test
    public void namesCarryUtcTimeAndRevision() {
        assertEquals("log-20261125T101507Z-1ed1b05002e4.txt",
                CrashLog.fileName("log", TIME, "1ed1b05002e4"));
        assertEquals("import-20261125T101507Z-1ed1b05002e4-dirty.txt",
                CrashLog.fileName("import", TIME, "1ed1b05002e4-dirty"));
        assertEquals("extract-19700101T000000Z-unversioned.txt",
                CrashLog.fileName("extract", 0, ""));
        assertEquals("log-20261125T101507Z-abc.txt", CrashLog.fileName("log", TIME, "a/b.c"));
    }
}

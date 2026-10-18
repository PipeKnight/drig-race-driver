package org.reassembly.grid;

/**
 * The import's last check (android_import.rs): derives the CD01, BP01 and
 * DF01 tables from the imported grid.exe as the game does at start. Loads
 * the game library on first use; MainActivity's later load is then a no-op.
 */
final class NativeCheck {
    static {
        System.loadLibrary("grid_android");
    }

    private NativeCheck() {}

    /** null when all three tables derive from gridDir/grid.exe; otherwise the error. */
    static native String deriveTables(String gridDir);
}

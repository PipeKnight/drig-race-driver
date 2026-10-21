// Decompile the functions containing the given addresses into <cache>/<ENTRY>.c (private, ignored).
// Arguments: <cache dir> <addr>... Writes <cache>/<ADDR>.miss when no function contains an address.
// Each output starts with a header naming the containing entry and the symbols.tsv plate, if any.
// @category GRID.Research
import java.io.File;
import java.io.PrintWriter;
import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileOptions;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;

public class DecompileCache extends GhidraScript {
    @Override public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 2) throw new IllegalArgumentException("Expected cache dir and addresses");
        File dir = new File(args[0]);
        dir.mkdirs();
        DecompInterface dec = new DecompInterface();
        dec.setOptions(new DecompileOptions());
        try {
            if (!dec.openProgram(currentProgram)) throw new IllegalStateException("Could not open program");
            for (int i = 1; i < args.length; i++) {
                long value = Long.parseUnsignedLong(args[i].replaceFirst("^0[xX]", ""), 16);
                Address addr = toAddr(value);
                Function fn = getFunctionContaining(addr);
                if (fn == null) {
                    try (PrintWriter w = new PrintWriter(new File(dir, String.format("%08X.miss", value)), "UTF-8")) { w.println("no function contains " + addr); }
                    println("DECOMPILE_MISS " + addr);
                    continue;
                }
                long entry = fn.getEntryPoint().getOffset();
                File out = new File(dir, String.format("%08X.c", entry));
                if (!out.exists()) {
                    DecompileResults r = dec.decompileFunction(fn, 120, monitor);
                    try (PrintWriter w = new PrintWriter(out, "UTF-8")) {
                        w.println("// Private inferred pseudocode from local/ghidra-master; not original source, do not commit.");
                        w.println(String.format("// Function %s at 0x%08X (%d bytes)", fn.getName(), entry, fn.getBody().getNumAddresses()));
                        String plate = getPlateComment(fn.getEntryPoint());
                        if (plate != null) for (String line : plate.split("\n")) w.println("// " + line);
                        if (r.decompileCompleted() && r.getDecompiledFunction() != null) w.println(r.getDecompiledFunction().getC());
                        else w.println("// FAILED: " + r.getErrorMessage());
                    }
                }
                if (entry != value) {
                    try (PrintWriter w = new PrintWriter(new File(dir, String.format("%08X.at", value)), "UTF-8")) { w.println(String.format("%08X", entry)); }
                }
                println(String.format("DECOMPILE_OK 0x%08X ENTRY=0x%08X %s", value, entry, out.getAbsolutePath()));
            }
        } finally { dec.dispose(); }
    }
}

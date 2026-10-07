// Targeted decompilation of independently identified runtime functions.
// Original/derived code output is written only to the caller's private directory.
// @category GRID.Research
import java.io.File;
import java.io.PrintWriter;
import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;

public class GridProbe extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 1) throw new IllegalArgumentException("Expected private output directory and optional entry addresses");
        File directory = new File(args[0]);
        directory.mkdirs();
        long[] entries = {0x004b3960L, 0x0049d760L, 0x00bf6e1aL, 0x00c01650L};
        if (args.length > 1) {
            entries = new long[args.length - 1];
            for (int i = 1; i < args.length; i++) entries[i - 1] = Long.parseUnsignedLong(args[i].replaceFirst("^0x", ""), 16);
        }
        DecompInterface decompiler = new DecompInterface();
        try {
            if (!decompiler.openProgram(currentProgram)) throw new IllegalStateException("Decompiler could not open target");
            for (long value : entries) {
                Address address = toAddr(value);
                disassemble(address);
                Function function = getFunctionAt(address);
                if (function == null) function = createFunction(address, "probe_" + Long.toHexString(value));
                if (function == null) throw new IllegalStateException("No function at " + address);
                DecompileResults result = decompiler.decompileFunction(function, 60, monitor);
                File output = new File(directory, Long.toHexString(value) + ".c.txt");
                try (PrintWriter writer = new PrintWriter(output, "UTF-8")) {
                    writer.println("// Private executable/memory reference; inferred pseudocode, not original source.");
                    writer.println("// Entry: " + address + "; language: " + currentProgram.getLanguageID());
                    if (result.decompileCompleted() && result.getDecompiledFunction() != null) {
                        writer.println(result.getDecompiledFunction().getC());
                    } else {
                        writer.println("// FAILED: " + result.getErrorMessage());
                        throw new IllegalStateException("Decompilation failed at " + address);
                    }
                }
                println("GRID_PROBE_OK " + address + " " + output.getAbsolutePath());
            }
        } finally {
            decompiler.dispose();
        }
    }
}

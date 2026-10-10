// Decompile with explicitly supplied ABI return contracts, then roll back the
// analysis transaction. This edits neither executable bytes nor stored types.
// @category GRID.Research
import java.io.File;
import java.io.PrintWriter;
import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.program.model.data.FloatDataType;
import ghidra.program.model.data.VoidDataType;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.SourceType;

public class GridReturnContracts extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 2 || args.length > 65) throw new IllegalArgumentException("Output directory and 1..64 address:float/void contracts required");
        File directory = new File(args[0]);
        directory.mkdirs();
        Function[] functions = new Function[args.length - 1];
        String[] before = new String[functions.length];
        String[] kinds = new String[functions.length];
        // Validate the entire request before changing any analysis types.
        for (int i = 0; i < functions.length; i++) {
            String[] parts = args[i + 1].split(":", -1);
            if (parts.length != 2 || !parts[0].matches("[0-9a-fA-F]{1,8}") || !(parts[1].equals("float") || parts[1].equals("void")))
                throw new IllegalArgumentException("Invalid return contract");
            functions[i] = getFunctionAt(toAddr(Long.parseUnsignedLong(parts[0], 16)));
            if (functions[i] == null) throw new IllegalArgumentException("Existing function required: " + parts[0]);
            before[i] = functions[i].getPrototypeString(false, false);
            kinds[i] = parts[1];
        }
        int transaction = currentProgram.startTransaction("Temporary observed ABI returns");
        DecompInterface decompiler = new DecompInterface();
        try {
            for (int i = 0; i < functions.length; i++)
                functions[i].setReturnType(kinds[i].equals("float") ? FloatDataType.dataType : VoidDataType.dataType, SourceType.USER_DEFINED);
            if (!decompiler.openProgram(currentProgram)) throw new IllegalStateException("Cannot open program");
            for (int i = 0; i < functions.length; i++) {
                Function function = functions[i];
                DecompileResults result = decompiler.decompileFunction(function, 60, monitor);
                if (!result.decompileCompleted() || result.getDecompiledFunction() == null)
                    throw new IllegalStateException("Decompilation failed: " + result.getErrorMessage());
                File output = new File(directory, Long.toHexString(function.getEntryPoint().getOffset()) + ".c.txt");
                try (PrintWriter writer = new PrintWriter(output, "UTF-8")) {
                    writer.println("// Private reference pseudocode, not original source.");
                    writer.println("// Temporary return contracts supplied from independent ABI evidence.");
                    writer.println("// Before: " + before[i]);
                    writer.println("// Requested: " + function.getPrototypeString(false, false));
                    writer.println("// Function body: " + function.getBody());
                    writer.println(result.getDecompiledFunction().getC());
                }
                println("GRID_CONTRACT_OK " + function.getEntryPoint() + " " + output.getAbsolutePath());
            }
        } finally {
            decompiler.dispose();
            currentProgram.endTransaction(transaction, false);
            println("GRID_CONTRACT_TRANSACTION_ROLLED_BACK");
        }
    }
}

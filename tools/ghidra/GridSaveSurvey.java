// Trace static references to ANSI path imports; preserve derived results privately.
// @category GRID.Research
import java.io.File;
import java.io.PrintWriter;
import java.util.HashSet;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.Symbol;
import ghidra.program.model.symbol.SymbolIterator;

public class GridSaveSurvey extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length != 1) throw new IllegalArgumentException("Expected private output directory");
        File directory = new File(args[0]);
        directory.mkdirs();
        int anchors = 0;
        int calls = 0;
        HashSet<Address> seen = new HashSet<>();
        try (PrintWriter writer = new PrintWriter(new File(directory, "save-path-imports.tsv"), "UTF-8")) {
            writer.println("# Static import references in partial analysis; not runtime observations.");
            SymbolIterator iterator = currentProgram.getSymbolTable().getAllSymbols(true);
            while (iterator.hasNext() && !monitor.isCancelled()) {
                Symbol symbol = iterator.next();
                String name = symbol.getName();
                if (!(name.contains("SHGetFolderPathA") || name.contains("CreateDirectoryA") ||
                      name.contains("GetFileAttributesA") || name.contains("CreateFileA") ||
                      name.contains("WideCharToMultiByte"))) continue;
                if (!seen.add(symbol.getAddress())) continue;
                anchors++;
                writer.println("IMPORT\t" + name + "\t" + symbol.getAddress());
                for (Reference ref : getReferencesTo(symbol.getAddress())) {
                    Function function = getFunctionContaining(ref.getFromAddress());
                    writer.println("REF\t" + ref.getFromAddress() + "\t" + ref.getReferenceType() + "\t" +
                        (function == null ? "unassigned" : function.getEntryPoint() + " " + function.getName()));
                    calls++;
                    if (function == null && currentProgram.getMemory().contains(ref.getFromAddress())) {
                        for (Reference indirect : getReferencesTo(ref.getFromAddress())) {
                            Function caller = getFunctionContaining(indirect.getFromAddress());
                            writer.println("IAT_REF\t" + indirect.getFromAddress() + "\t" + indirect.getReferenceType() + "\t" +
                                (caller == null ? "unassigned" : caller.getEntryPoint() + " " + caller.getName()));
                            calls++;
                        }
                    }
                }
            }
            writer.println("# Anchors=" + anchors + " references=" + calls);
        }
        println("GRID_SAVE_SURVEY_OK anchors=" + anchors + " references=" + calls);
    }
}

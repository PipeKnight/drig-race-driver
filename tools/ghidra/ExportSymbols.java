// Dump user-defined names and all comments of the current program to TSV for tools/re/merge_symbols.py.
// Arguments: <output.tsv> [source label]. Columns: address, name, kind, comment_type, comment, source.
// Comments written by ImportSymbols ("[symbols.tsv]" plates) are left out; they came from the TSV.
// @category GRID.Research
import java.io.File;
import java.io.PrintWriter;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressIterator;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.model.symbol.Symbol;
import ghidra.program.model.symbol.SymbolIterator;
import ghidra.program.model.symbol.SymbolType;

public class ExportSymbols extends GhidraScript {
    @Override public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 1) throw new IllegalArgumentException("Expected output TSV path");
        String source = args.length > 1 ? args[1] : currentProgram.getDomainFile().getProjectLocator().getName();
        File out = new File(args[0]);
        if (out.getParentFile() != null) out.getParentFile().mkdirs();
        int names = 0, comments = 0;
        try (PrintWriter w = new PrintWriter(out, "UTF-8")) {
            w.println("address\tname\tkind\tcomment_type\tcomment\tsource");
            SymbolIterator it = currentProgram.getSymbolTable().getAllSymbols(true);
            while (it.hasNext() && !monitor.isCancelled()) {
                Symbol s = it.next();
                if (s.getSource() != SourceType.USER_DEFINED || s.isExternal()) continue;
                if (!s.getAddress().isMemoryAddress()) continue;
                SymbolType t = s.getSymbolType();
                String kind = t == SymbolType.FUNCTION ? "func" : (t == SymbolType.LABEL ? "global" : null);
                if (kind == null) continue;
                if (kind.equals("global") && getFunctionAt(s.getAddress()) != null) kind = "func";
                w.println(hex(s.getAddress()) + "\t" + s.getName(true) + "\t" + kind + "\t\t\t" + source);
                names++;
            }
            for (Function fn : currentProgram.getFunctionManager().getFunctions(true)) {
                if (fn.getSignatureSource() == SourceType.USER_DEFINED && fn.getSymbol().getSource() != SourceType.USER_DEFINED) {
                    w.println(hex(fn.getEntryPoint()) + "\t" + fn.getName() + "\tfunc\tsignature\t" + esc(fn.getPrototypeString(false, false)) + "\t" + source);
                    names++;
                }
            }
            AddressIterator ci = currentProgram.getListing().getCommentAddressIterator(currentProgram.getMemory(), true);
            while (ci.hasNext() && !monitor.isCancelled()) {
                Address a = ci.next();
                comments += emit(w, a, "plate", getPlateComment(a), source);
                comments += emit(w, a, "pre", getPreComment(a), source);
                comments += emit(w, a, "eol", getEOLComment(a), source);
                comments += emit(w, a, "post", getPostComment(a), source);
                comments += emit(w, a, "repeatable", getRepeatableComment(a), source);
            }
        }
        println("EXPORT_SYMBOLS names=" + names + " comments=" + comments + " " + out.getAbsolutePath());
    }

    int emit(PrintWriter w, Address a, String type, String text, String source) {
        if (text == null || text.isBlank() || text.startsWith("[symbols.tsv]")) return 0;
        w.println(hex(a) + "\t\t\t" + type + "\t" + esc(text) + "\t" + source);
        return 1;
    }

    static String hex(Address a) { return String.format("0x%08X", a.getOffset()); }

    static String esc(String s) { return s.replace("\\", "\\\\").replace("\t", "\\t").replace("\r", "").replace("\n", "\\n"); }
}

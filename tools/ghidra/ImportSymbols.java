// Apply docs/re/symbols.tsv names and a provenance plate comment to the current program.
// Arguments: <symbols.tsv> [report.txt]. Never overwrites a user-defined name that differs
// (reported as a conflict) and only replaces plate comments it wrote itself.
// @category GRID.Research
import java.io.BufferedReader;
import java.io.File;
import java.io.FileInputStream;
import java.io.InputStreamReader;
import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.util.HashMap;
import java.util.Map;
import ghidra.app.cmd.disassemble.DisassembleCommand;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.model.symbol.Symbol;
import ghidra.program.model.symbol.SymbolTable;

public class ImportSymbols extends GhidraScript {
    static final String MARK = "[symbols.tsv]";

    @Override public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 1) throw new IllegalArgumentException("Expected symbols.tsv path");
        PrintWriter report = args.length > 1 ? new PrintWriter(new File(args[1]), "UTF-8") : null;
        SymbolTable table = currentProgram.getSymbolTable();
        int renamed = 0, created = 0, labels = 0, kept = 0, skipped = 0, comments = 0;
        try (BufferedReader in = new BufferedReader(new InputStreamReader(new FileInputStream(args[0]), StandardCharsets.UTF_8))) {
            String header = in.readLine();
            Map<String, Integer> col = new HashMap<>();
            String[] names = header.split("\t", -1);
            for (int i = 0; i < names.length; i++) col.put(names[i], i);
            String line;
            while ((line = in.readLine()) != null) {
                if (monitor.isCancelled()) break;
                String[] f = line.split("\t", -1);
                Address addr = toAddr(Long.parseUnsignedLong(f[col.get("address")].replaceFirst("^0[xX]", ""), 16));
                String name = sanitize(f[col.get("name")]);
                String kind = f[col.get("kind")];
                MemoryBlock block = currentProgram.getMemory().getBlock(addr);
                if (block == null) { skipped++; log(report, "SKIP " + addr + " outside memory"); continue; }
                Symbol primary = table.getPrimarySymbol(addr);
                boolean userNamed = primary != null && primary.getSource() == SourceType.USER_DEFINED;
                if (name.isEmpty()) {
                    // unnamed rows only get the provenance plate comment below
                } else if (kind.equals("func") && block.isExecute()) {
                    Function fn = getFunctionAt(addr);
                    if (fn == null && getFunctionContaining(addr) == null && currentProgram.getListing().getInstructionContaining(addr) == null) {
                        if (getInstructionAt(addr) == null) new DisassembleCommand(addr, null, true).applyTo(currentProgram, monitor);
                        Instruction insn = getInstructionAt(addr);
                        if (insn != null) { fn = createFunction(addr, name); if (fn != null) created++; }
                    }
                    if (fn != null) {
                        if (fn.getSignatureSource() == SourceType.USER_DEFINED || (fn.getSymbol().getSource() == SourceType.USER_DEFINED && !fn.getName().equals(name))) {
                            if (!fn.getName().equals(name)) { kept++; log(report, "KEEP " + addr + " existing=" + fn.getName() + " tsv=" + name); }
                        } else if (!fn.getName().equals(name)) { fn.setName(name, SourceType.USER_DEFINED); renamed++; }
                    } else { addLabel(table, addr, name); labels++; log(report, "LABEL " + addr + " " + name + " (interior or not code)"); }
                } else {
                    if (userNamed && !primary.getName().equals(name)) { kept++; log(report, "KEEP " + addr + " existing=" + primary.getName() + " tsv=" + name); addLabel(table, addr, name); }
                    else if (!userNamed || !primary.getName().equals(name)) { Symbol s = table.createLabel(addr, name, SourceType.USER_DEFINED); s.setPrimary(); labels++; }
                }
                String plate = MARK + " " + f[col.get("name")] + " | " + f[col.get("kind")] + " | " + f[col.get("confidence")]
                    + " | " + f[col.get("entry")] + " | doc " + f[col.get("doc")] + " | port " + f[col.get("port")];
                String extra = col.containsKey("comment") && col.get("comment") < f.length ? f[col.get("comment")] : "";
                if (!extra.isEmpty()) plate += "\n" + extra;
                String old = getPlateComment(addr);
                String merged;
                if (old == null || old.isEmpty()) merged = plate;
                else if (old.startsWith(MARK)) { int nl = old.indexOf("\n" + "--\n"); merged = plate + (nl >= 0 ? old.substring(nl) : ""); }
                else merged = plate + "\n--\n" + old;
                if (!merged.equals(old)) { setPlateComment(addr, merged); comments++; }
            }
        } finally {
            String summary = "IMPORT_SYMBOLS renamed=" + renamed + " created=" + created + " labels=" + labels + " kept=" + kept + " skipped=" + skipped + " comments=" + comments;
            println(summary);
            if (report != null) { report.println(summary); report.close(); }
        }
    }

    void addLabel(SymbolTable table, Address addr, String name) throws Exception {
        for (Symbol s : table.getSymbols(addr)) if (s.getName().equals(name)) return;
        table.createLabel(addr, name, SourceType.USER_DEFINED);
    }

    static String sanitize(String name) {
        return name.trim().replaceAll("[^A-Za-z0-9_:.@$?<>-]", "_");
    }

    static void log(PrintWriter report, String text) { if (report != null) report.println(text); }
}

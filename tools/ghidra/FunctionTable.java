// Write every analysed function as TSV (entry, last address, name, name source) for tools/re.
// Arguments: <output.tsv>. Output is derived from the executable: keep it in ignored local/.
// @category GRID.Research
import java.io.File;
import java.io.PrintWriter;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;

public class FunctionTable extends GhidraScript {
    @Override public void run() throws Exception {
        File out = new File(getScriptArgs()[0]);
        if (out.getParentFile() != null) out.getParentFile().mkdirs();
        int n = 0;
        try (PrintWriter w = new PrintWriter(out, "UTF-8")) {
            w.println("entry\tend\tname\tsource");
            for (Function fn : currentProgram.getFunctionManager().getFunctions(true)) {
                if (fn.isExternal() || fn.isThunk() && fn.getBody().isEmpty()) continue;
                w.println(String.format("0x%08X\t0x%08X\t%s\t%s", fn.getEntryPoint().getOffset(),
                    fn.getBody().getMaxAddress().getOffset(), fn.getName(), fn.getSymbol().getSource()));
                n++;
            }
        }
        println("FUNCTION_TABLE functions=" + n + " " + out.getAbsolutePath());
    }
}

// Survey resource strings/xrefs after a bounded PE analysis. No guessed loaders.
// @category GRID.Research
import java.io.File;
import java.io.PrintWriter;
import java.util.Locale;
import java.nio.charset.StandardCharsets;
import java.util.HashSet;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Data;
import ghidra.program.model.listing.DataIterator;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.address.Address;

public class GridLoaderSurvey extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length != 1) throw new IllegalArgumentException("Expected private output directory");
        File directory = new File(args[0]);
        directory.mkdirs();
        int candidates = 0;
        int references = 0;
        HashSet<Address> seen = new HashSet<>();
        try (PrintWriter writer = new PrintWriter(new File(directory, "loader-survey.tsv"), "UTF-8")) {
            writer.println("# Bounded auto-analysis; strings/xrefs are candidates, not confirmed loaders.");
            writer.println("# Language=" + currentProgram.getLanguageID() + " base=" + currentProgram.getImageBase());
            writer.println("# Functions=" + currentProgram.getFunctionManager().getFunctionCount());
            DataIterator iterator = currentProgram.getListing().getDefinedData(true);
            while (iterator.hasNext() && !monitor.isCancelled()) {
                Data data = iterator.next();
                Object value = data.getValue();
                if (!(value instanceof String)) continue;
                String text = (String)value;
                String lower = text.toLowerCase(Locale.ROOT);
                if (!(lower.contains("pssg") || lower.contains(".jpk") ||
                      lower.contains("car_loader") || lower.contains("track_loader") ||
                      lower.contains("datablock") || lower.contains("rendersource"))) continue;
                candidates++;
                seen.add(data.getAddress());
                writer.println("STRING\t" + data.getAddress() + "\t" + text.replace('\n', ' ').replace('\r', ' ').replace('\t', ' '));
                for (Reference reference : getReferencesTo(data.getAddress())) {
                    Function function = getFunctionContaining(reference.getFromAddress());
                    writer.println("XREF\t" + reference.getFromAddress() + "\t" + reference.getReferenceType() + "\t" +
                        (function == null ? "unassigned" : function.getEntryPoint() + " " + function.getName()));
                    references++;
                }
            }
            // The string analyzer may not run before the analysis deadline.
            // Scan bounded initialized blocks for printable ASCII runs instead.
            for (MemoryBlock block : currentProgram.getMemory().getBlocks()) {
                if (!block.isInitialized() || block.getSize() > 64L * 1024 * 1024) continue;
                byte[] bytes = new byte[(int)block.getSize()];
                int size = block.getBytes(block.getStart(), bytes);
                int start = 0;
                while (start < size && !monitor.isCancelled()) {
                    while (start < size && (bytes[start] < 32 || bytes[start] > 126)) start++;
                    int end = start;
                    while (end < size && bytes[end] >= 32 && bytes[end] <= 126) end++;
                    if (end - start >= 4 && end - start <= 4096) {
                        String text = new String(bytes, start, end - start, StandardCharsets.US_ASCII);
                        String lower = text.toLowerCase(Locale.ROOT);
                        if (lower.contains("pssg") || lower.contains(".jpk") || lower.contains("car_loader") ||
                            lower.contains("track_loader") || lower.contains("datablock") || lower.contains("rendersource")) {
                            Address address = block.getStart().add(start);
                            if (seen.add(address)) {
                                candidates++;
                                writer.println("ASCII_CANDIDATE\t" + address + "\t" + text);
                                for (Reference reference : getReferencesTo(address)) {
                                    Function function = getFunctionContaining(reference.getFromAddress());
                                    writer.println("XREF\t" + reference.getFromAddress() + "\t" + reference.getReferenceType() + "\t" +
                                        (function == null ? "unassigned" : function.getEntryPoint() + " " + function.getName()));
                                    references++;
                                }
                            }
                        }
                    }
                    start = end + 1;
                }
            }
            writer.println("# Candidate strings=" + candidates + " references=" + references);
        }
        println("GRID_LOADER_SURVEY_OK strings=" + candidates + " references=" + references +
            " functions=" + currentProgram.getFunctionManager().getFunctionCount());
    }
}

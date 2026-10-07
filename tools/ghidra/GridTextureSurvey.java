// Identify texture-name and sampler references in the private unpacked PE.
// @category GRID.Research
import java.io.File;
import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.util.HashSet;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.listing.Function;
import ghidra.program.model.address.Address;
import ghidra.program.model.symbol.Reference;
import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;

public class GridTextureSurvey extends GhidraScript {
    public void run() throws Exception {
        File out=new File(getScriptArgs()[0]); out.mkdirs();
        HashSet<Function> functions=new HashSet<>();
        try (PrintWriter w=new PrintWriter(new File(out,"texture-xrefs.tsv"),"UTF-8")) {
            for (MemoryBlock block:currentProgram.getMemory().getBlocks()) {
                if (!block.isInitialized() || block.getSize()>64L*1024*1024) continue;
                byte[] b=new byte[(int)block.getSize()];int length=block.getBytes(block.getStart(),b);
                int start=0;
                while (start<length) {
                    while(start<length && (b[start]<32 || b[start]>126)) start++;
                    int end=start;while(end<length && b[end]>=32 && b[end]<=126)end++;
                    if(end-start>=4 && end-start<4096) {
                        String text=new String(b,start,end-start,StandardCharsets.US_ASCII);
                        if(text.contains("TDiffuseAlphaMap") || text.contains("_wheel_tm") || text.contains("_tread_tm") || text.equals("Car Body Paint") || text.equals("Tyre Wall") || text.equals("Window Glass")) {
                            Address address=block.getStart().add(start);w.println("STRING\t"+address+"\t"+text);
                            for(Reference r:getReferencesTo(address)) {
                                Function f=getFunctionContaining(r.getFromAddress());
                                w.println("XREF\t"+r.getFromAddress()+"\t"+(f==null?"unassigned":f.getEntryPoint()));
                                if(f!=null)functions.add(f);
                            }
                        }
                    }
                    start=end+1;
                }
            }
        }
        DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
        try {
            int count=0;
            for(Function f:functions) {
                if(count++>=20)break;
                DecompileResults r=d.decompileFunction(f,30,monitor);
                try(PrintWriter w=new PrintWriter(new File(out,"function-"+f.getEntryPoint()+".private.c"),"UTF-8")) {
                    if(r.decompileCompleted())w.print(r.getDecompiledFunction().getC());else w.println("FAILED "+r.getErrorMessage());
                }
            }
        } finally {d.dispose();}
        println("Texture consumer candidate functions="+functions.size());
    }
}

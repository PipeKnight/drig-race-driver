// Resolve an interior instruction to its existing analyzed function.
// Never creates a function at a guessed interior address; output stays private.
// @category GRID.Research
import java.io.File;
import java.io.PrintWriter;
import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
public class GridContaining extends GhidraScript {
    @Override public void run() throws Exception {
        String[] args=getScriptArgs();
        if(args.length<2||args.length>65)throw new IllegalArgumentException("Expected private output directory and 1..64 interior addresses");
        File directory=new File(args[0]);directory.mkdirs();
        DecompInterface decompiler=new DecompInterface();
        try {
            if(!decompiler.openProgram(currentProgram))throw new IllegalStateException("Could not open program");
            for(int i=1;i<args.length;i++) {
                long value=Long.parseUnsignedLong(args[i].replaceFirst("^0x",""),16);
                Address address=toAddr(value);Function function=getFunctionContaining(address);
                if(function==null)throw new IllegalStateException("No analyzed function contains "+address);
                DecompileResults result=decompiler.decompileFunction(function,60,monitor);
                File output=new File(directory,Long.toHexString(value)+".c.txt");
                try(PrintWriter writer=new PrintWriter(output,"UTF-8")) {
                    writer.println("// Private inferred pseudocode, not original source.");
                    writer.println("// Interior: "+address+"; containing entry: "+function.getEntryPoint());
                    if(result.decompileCompleted()&&result.getDecompiledFunction()!=null)writer.println(result.getDecompiledFunction().getC());
                    else {writer.println("// FAILED: "+result.getErrorMessage());throw new IllegalStateException("Decompilation failed at "+function.getEntryPoint());}
                }
                println("GRID_CONTAINING_OK "+address+" ENTRY="+function.getEntryPoint()+" "+output.getAbsolutePath());
            }
        } finally {decompiler.dispose();}
    }
}

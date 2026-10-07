// Target static car-texture tuple consumers; outputs stay in private evidence.
// @category GRID.Research
import java.io.*;
import java.util.*;
import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.*;
import ghidra.program.model.listing.Function;
public class GridTableConsumers extends GhidraScript {
 public void run() throws Exception {
  File directory=new File(getScriptArgs()[0]); directory.mkdirs();
  Set<Function> functions=new LinkedHashSet<>();
  for(long address:new long[]{0x9ca918L,0x9ca9b8L,0x9caca4L}) {
   Function function=getFunctionContaining(toAddr(address));
   if(function==null)throw new IllegalStateException("No existing function contains "+Long.toHexString(address));
   functions.add(function);functions.addAll(function.getCalledFunctions(monitor));
  }
  DecompInterface decompiler=new DecompInterface();
  try {
   if(!decompiler.openProgram(currentProgram))throw new IllegalStateException("Cannot open program");
   for(Function function:functions) {
    DecompileResults result=decompiler.decompileFunction(function,30,monitor);
    try(PrintWriter writer=new PrintWriter(new File(directory,function.getEntryPoint()+".private.c"),"UTF-8")) {
     writer.println("// Private inferred pseudocode; not original source. Entry "+function.getEntryPoint());
     if(result.decompileCompleted()&&result.getDecompiledFunction()!=null)writer.println(result.getDecompiledFunction().getC());
     else writer.println("// FAILED "+result.getErrorMessage());
    }
   }
   println("Texture tuple consumer survey functions="+functions.size());
  }finally{decompiler.dispose();}
 }
}

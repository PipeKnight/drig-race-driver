// Inspect existing function boundaries/xrefs near the identified slot reader.
// @category GRID.Research
import java.io.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.Reference;
public class GridSlotNeighborhood extends GhidraScript {
 public void run() throws Exception {
  File file=new File(getScriptArgs()[0]);
  if(file.exists())throw new IllegalStateException("Refusing overwrite");
  try(PrintWriter writer=new PrintWriter(file,"UTF-8")) {
   FunctionIterator functions=currentProgram.getFunctionManager().getFunctions(toAddr(0x90a800),true);
   while(functions.hasNext()) {
    Function f=functions.next(); if(f.getEntryPoint().getOffset()>0x90d000)break;
    writer.println("F\t"+f.getEntryPoint()+"\t"+f.getBody().getMaxAddress()+"\t"+f.getName());
    for(Function called:f.getCalledFunctions(monitor))writer.println("C\t"+f.getEntryPoint()+"\t"+called.getEntryPoint());
    for(Reference ref:getReferencesTo(f.getEntryPoint())) {
     Function caller=getFunctionContaining(ref.getFromAddress());
     writer.println("R\t"+f.getEntryPoint()+"\t"+ref.getFromAddress()+"\t"+ref.getReferenceType()+"\t"+(caller==null?"data":caller.getEntryPoint()));
    }
   }
  }
  println("GRID_SLOT_NEIGHBORHOOD_OK "+file);
 }
}

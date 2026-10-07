// Follow the observed per-vehicle grid-index field in existing disassembly.
// @category GRID.Research
import java.io.*;
import java.util.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import ghidra.program.model.scalar.Scalar;
public class GridSpawnRefs extends GhidraScript {
 public void run() throws Exception {
  File output=new File(getScriptArgs()[0]);if(output.exists())throw new IllegalStateException("Refusing overwrite");
  Set<Function> functions=new LinkedHashSet<>();int matches=0;
  try(PrintWriter writer=new PrintWriter(output,"UTF-8")) {
   InstructionIterator iterator=currentProgram.getListing().getInstructions(true);
   while(iterator.hasNext()&&!monitor.isCancelled()) {
    Instruction instruction=iterator.next(); boolean hit=false;
    for(int i=0;i<instruction.getNumOperands();i++)for(Object object:instruction.getOpObjects(i))
     if(object instanceof Scalar&&((Scalar)object).getUnsignedValue()==0x2e4c)hit=true;
    if(hit) {
     Function function=getFunctionContaining(instruction.getAddress());matches++;
     if(function!=null)functions.add(function);
     writer.println("I\t"+instruction.getAddress()+"\t"+(function==null?"unassigned":function.getEntryPoint())+"\t"+instruction);
    }
   }
   for(Function function:functions)for(Function called:function.getCalledFunctions(monitor))writer.println("C\t"+function.getEntryPoint()+"\t"+called.getEntryPoint());
   writer.println("# matches="+matches+" functions="+functions.size());
  }
  println("GRID_SPAWN_REFS_OK matches="+matches+" functions="+functions.size());
 }
}

// Bounded static string/xref survey; results are candidates, not runtime proof.
// @category GRID.Research
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.util.Locale;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;

public class GridBehaviorSurvey extends GhidraScript {
 public void run() throws Exception {
  File directory=new File(getScriptArgs()[0]);
  if(!directory.mkdirs())throw new IllegalStateException("Output must be new");
  String[] terms={"grid_start", "slot_", "progress_track", "total_distance", "num_gates", "tyretable", "physparams", "csv", "timestep", "time_step", "simulation", "suspension", "wheelbase", "car_mass", "inertia", "queryperformance", "fixedstep", "standing", "rolling", "grid", "ctf", "mass", "gearratio", "spring", "damping", "forwards", "vehicleparameter", "drivetrain", "torque", "collision", "cqt", "performancefile"};
  int matches=0, refs=0;
  try(PrintWriter writer=new PrintWriter(new File(directory,"strings.tsv"),"UTF-8")) {
   writer.println("# Private candidate strings/references; no runtime behavior claim");
   for(MemoryBlock block:currentProgram.getMemory().getBlocks()) {
    if(!block.isInitialized()||block.getSize()>64L*1024*1024)continue;
    byte[] bytes=new byte[(int)block.getSize()]; int size=block.getBytes(block.getStart(),bytes);
    for(int start=0;start<size&&!monitor.isCancelled();) {
     while(start<size&&(bytes[start]<32||bytes[start]>126))start++;
     int end=start; while(end<size&&bytes[end]>=32&&bytes[end]<=126)end++;
     if(end-start>=4&&end-start<=4096) {
      String value=new String(bytes,start,end-start,StandardCharsets.US_ASCII), lower=value.toLowerCase(Locale.ROOT);
      boolean selected=false; for(String term:terms)if(lower.contains(term)){selected=true;break;}
      if(selected) {
       Address address=block.getStart().add(start); matches++;
       writer.println("S\t"+address+"\t"+value);
       for(Reference ref:getReferencesTo(address)) {
        Function function=getFunctionContaining(ref.getFromAddress()); refs++;
        writer.println("R\t"+address+"\t"+ref.getFromAddress()+"\t"+ref.getReferenceType()+"\t"+(function==null?"unassigned":function.getEntryPoint()+" "+function.getName()));
       }
      }
     }
     start=end+1;
    }
   }
   writer.println("# matches="+matches+" references="+refs);
  }
  println("GRID_BEHAVIOR_SURVEY_OK strings="+matches+" references="+refs);
 }
}

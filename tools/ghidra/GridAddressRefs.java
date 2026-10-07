// Static references to observed addresses; outputs only to private caller directory.
// @category GRID.Research
import java.io.File;
import java.io.PrintWriter;
import java.util.ArrayList;
import java.util.Collections;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.symbol.Reference;

public class GridAddressRefs extends GhidraScript {
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 2) throw new IllegalArgumentException("Private directory and observed addresses required");
        File directory = new File(args[0]);
        directory.mkdirs();
        File output = new File(directory, "address-refs.tsv");
        if (output.exists()) throw new IllegalStateException("Refusing evidence replacement");
        try (PrintWriter writer = new PrintWriter(output, "UTF-8");
             PrintWriter contexts = new PrintWriter(new File(directory,"contexts.tsv"),"UTF-8")) {
            writer.println("target\tfrom\ttype\tfunction\tinstruction");
            for (int i=1; i<args.length; i++) {
                Address target = toAddr(Long.parseUnsignedLong(args[i].replaceFirst("^0x", ""),16));
                for (Reference reference : getReferencesTo(target)) {
                    Address from = reference.getFromAddress();
                    Function function = getFunctionContaining(from);
                    Instruction instruction = getInstructionAt(from);
                    writer.println(target+"\t"+from+"\t"+reference.getReferenceType()+"\t"+
                        (function==null ? "" : function.getEntryPoint())+"\t"+
                        (instruction==null ? "" : instruction.toString()));
                    if(instruction!=null) {
                        ArrayList<Instruction> neighborhood=new ArrayList<Instruction>();
                        Instruction cursor=instruction;
                        for(int n=0;n<8 && cursor!=null;n++) {
                            if(function!=null && !function.getBody().contains(cursor.getAddress())) break;
                            neighborhood.add(cursor);
                            cursor=currentProgram.getListing().getInstructionBefore(cursor.getAddress());
                        }
                        Collections.reverse(neighborhood);
                        for(Instruction item:neighborhood) contexts.println(target+"\t"+from+"\t"+item.getAddress()+"\t"+item.toString());
                    }
                }
            }
        }
        println("GRID_ADDRESS_REFS_OK "+output.getAbsolutePath());
    }
}

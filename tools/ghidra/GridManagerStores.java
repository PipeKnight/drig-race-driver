// @category GRID.Research
import java.io.File;
import java.io.PrintWriter;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
import ghidra.program.model.listing.Function;

public class GridManagerStores extends GhidraScript {
    public void run() throws Exception {
        String[] args = getScriptArgs();
        File output = new File(args[0]);
        if (output.exists()) throw new IllegalStateException("Refusing evidence replacement");
        try (PrintWriter writer = new PrintWriter(output, "UTF-8")) {
            writer.println("address\tfunction\tinstruction");
            InstructionIterator instructions = currentProgram.getListing().getInstructions(true);
            while (instructions.hasNext()) {
                monitor.checkCancelled();
                Instruction instruction = instructions.next();
                if (!instruction.getMnemonicString().startsWith("MOV") &&
                    !instruction.getMnemonicString().equals("FSTP")) continue;
                String destination = instruction.getDefaultOperandRepresentation(0);
                if (!destination.contains("[")) continue;
                boolean matched = args.length == 1 && destination.contains("0x218c");
                for (int i=1;i<args.length;i++) if(destination.contains(args[i])) matched=true;
                if (!matched) continue;
                Function function = getFunctionContaining(instruction.getAddress());
                writer.println(instruction.getAddress()+"\t"+
                    (function == null ? "" : function.getEntryPoint())+"\t"+instruction);
            }
        }
        println("GRID_MANAGER_STORES_OK "+output.getAbsolutePath());
    }
}

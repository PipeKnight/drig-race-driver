// Candidate literal-offset uses, including LEA and pointer setup; identity unproven.
// @category GRID.Research
import java.io.File;
import java.io.PrintWriter;
import java.util.regex.Pattern;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
import ghidra.program.model.listing.Function;

public class GridDisplacementRefs extends GhidraScript {
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 2 || args.length > 33) throw new IllegalArgumentException("Private output and 1..32 offsets required");
        File output = new File(args[0]);
        if (output.exists()) throw new IllegalStateException("Refusing evidence replacement");
        Pattern[] patterns = new Pattern[args.length-1];
        for (int i=1;i<args.length;i++) {
            if (!args[i].matches("0x[0-9a-f]{1,6}")) throw new IllegalArgumentException("Invalid literal offset");
            patterns[i-1] = Pattern.compile("(?i)(?<![0-9a-f])"+Pattern.quote(args[i])+"(?![0-9a-f])");
        }
        int rows = 0;
        try (PrintWriter writer = new PrintWriter(output,"UTF-8")) {
            writer.println("offset\taddress\tfunction\toperand\tinstruction");
            InstructionIterator instructions = currentProgram.getListing().getInstructions(true);
            while (instructions.hasNext()) {
                monitor.checkCancelled();
                Instruction instruction = instructions.next();
                for (int operand=0;operand<instruction.getNumOperands();operand++) {
                    String text = instruction.getDefaultOperandRepresentation(operand);
                    for (int i=0;i<patterns.length;i++) {
                        if (!patterns[i].matcher(text).find()) continue;
                        if (++rows>2048) throw new IllegalStateException("Reference limit exceeded");
                        Function function=getFunctionContaining(instruction.getAddress());
                        writer.println(args[i+1]+"\t"+instruction.getAddress()+"\t"+
                            (function==null?"":function.getEntryPoint())+"\t"+operand+"\t"+instruction);
                    }
                }
            }
        }
        println("GRID_DISPLACEMENT_REFS_OK "+rows+" "+output.getAbsolutePath());
    }
}

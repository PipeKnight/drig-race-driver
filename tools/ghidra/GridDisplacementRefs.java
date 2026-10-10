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
        if (args.length < 2 || args.length > 34) throw new IllegalArgumentException("Private output and 1..32 offsets required");
        File output = new File(args[0]);
        if (output.exists()) throw new IllegalStateException("Refusing evidence replacement");
        int end = args.length;
        long minimum = 0, maximum = 0xffffffffL;
        boolean scoped = args[end-1].startsWith("functions:");
        if (scoped) {
            String range=args[--end].substring(10);
            if (!range.matches("[0-9a-f]{6,8}-[0-9a-f]{6,8}")) throw new IllegalArgumentException("Invalid function range");
            String[] limits=range.split("-");
            minimum=Long.parseUnsignedLong(limits[0],16);maximum=Long.parseUnsignedLong(limits[1],16);
            if(minimum>=maximum)throw new IllegalArgumentException("Empty function range");
        }
        if(end<2||end>33)throw new IllegalArgumentException("1..32 offsets required");
        Pattern[] patterns = new Pattern[end-1];
        for (int i=1;i<end;i++) {
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
                Function function=getFunctionContaining(instruction.getAddress());
                if(scoped && (function==null || function.getEntryPoint().getOffset()<minimum || function.getEntryPoint().getOffset()>=maximum))continue;
                for (int operand=0;operand<instruction.getNumOperands();operand++) {
                    String text = instruction.getDefaultOperandRepresentation(operand);
                    for (int i=0;i<patterns.length;i++) {
                        if (!patterns[i].matcher(text).find()) continue;
                        if (++rows>2048) throw new IllegalStateException("Reference limit exceeded");
                        writer.println(args[i+1]+"\t"+instruction.getAddress()+"\t"+
                            (function==null?"":function.getEntryPoint())+"\t"+operand+"\t"+instruction);
                    }
                }
            }
        }
        println("GRID_DISPLACEMENT_REFS_OK "+rows+" "+output.getAbsolutePath());
    }
}

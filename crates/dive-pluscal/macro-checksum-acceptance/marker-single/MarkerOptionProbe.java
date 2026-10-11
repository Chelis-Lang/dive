// Development-only pinned translator adapter. Never linked into dive.
package pcal;
import java.nio.file.*;
import java.nio.charset.StandardCharsets;
import java.util.*;
import pcal.*;
import java.lang.reflect.*;
import pcal.exception.ParseAlgorithmException;

public class MarkerOptionProbe {
    public static void main(String[] args) throws Exception {
        String filename = args[args.length - 1];
        {
            try {
                PcalParams.resetParams(); AST.indentDepth=0;
                if (trans.parseAndProcessArguments(args) != trans.STATUS_OK) throw new IllegalArgumentException("reference option parsing failed");
                String text = Files.readString(Path.of(filename));
                Vector<String> lines = trans.removeTabs(Arrays.asList(text.split("\\r\\n|\\r|\\n", -1)));
                IntPair loc = new IntPair(0,0); int sourceStatus=trans.STATUS_OK; boolean found=false;
                while(!found) {
                    try {
                        ParseAlgorithm.FindToken("PlusCal",lines,loc,"");
                        String next=ParseAlgorithm.GotoNextNonSpace(lines,loc).substring(loc.two);
                        if(next.startsWith("options") && ParseAlgorithm.NextNonIdChar(next,0)==7) {
                            PcalParams.optionsInFile=true; sourceStatus=ParseAlgorithm.ProcessOptions(lines,loc);found=true;
                        }
                    } catch(ParseAlgorithmException error) {break;}
                }
                System.out.println("SOURCE_CONTEXT\t"+sourceStatus+"\t"+found+"\t"+encode(json(Map.of("fairness",PcalParams.FairnessOption,"label",PcalParams.LabelFlag,"root",PcalParams.LabelRoot))));
                int line=-1, column=-1; boolean fair=false;
                for(int i=0;i<lines.size() && line<0;i++) {
                    int index=lines.get(i).indexOf("--algorithm");
                    if(index>=0){line=i;column=index+11;}
                    else {index=lines.get(i).indexOf("--fair");if(index>=0){line=i;column=index+6;fair=true;}}
                }
                if(line<0)throw new IllegalArgumentException("no algorithm marker");
                ParseAlgorithm.uncomment(lines,line,column);
                TLAtoPCalMapping mapping=new TLAtoPCalMapping();mapping.algLine=line;mapping.algColumn=column;PcalParams.tlaPcalMapping=mapping;
                PcalCharReader reader=new PcalCharReader(lines,line,column,lines.size(),0);
                AST ast=ParseAlgorithm.getAlgorithm(reader,fair);
                emit("parse",ast,fair);
                PCalTLAGenerator generator=new PCalTLAGenerator(ast);generator.removeNameConflicts();
                emit("names",ast,fair);
                generator.translate();emit("translated",ast,fair);
            }catch(Throwable error){System.out.println("REJECTED\t"+encode(error.getClass().getName()+": "+error.getMessage()));}
        }
    }
    private static void emit(String stage,AST ast,boolean fair) throws Exception {
        AST.indentDepth=0;String serialized=ast.toString();
        System.out.println("STAGE\t"+stage+"\t"+Validator.checksum(fair?"fair":serialized)+"\t"+encode(serialized)+"\t"+encode(json(ast)));
    }
    private static String encode(String text) {
        return Base64.getEncoder().encodeToString(text.getBytes(StandardCharsets.UTF_8));
    }
    private static String json(Object value) throws Exception {
        if (value == null) return "null";
        if (value instanceof String || value instanceof Character) return quote(value.toString());
        if (value instanceof Number || value instanceof Boolean) return value.toString();
        if (value instanceof PCalLocation) {
            PCalLocation location = (PCalLocation)value;
            return "{\"line\":" + location.getLine() + ",\"column\":" + location.getColumn() + "}";
        }
        if (value instanceof Region) {
            Region region = (Region)value;
            return "{\"begin\":" + json(region.getBegin()) + ",\"end\":" + json(region.getEnd()) + "}";
        }
        if (value instanceof Collection) {
            List<String> items = new ArrayList<>();
            for (Object item : (Collection<?>) value) items.add(json(item));
            return "[" + String.join(",", items) + "]";
        }
        Map<String, Object> fields = new TreeMap<>();
        if (value instanceof Map) fields.putAll((Map<String, Object>) value);
        else {
            fields.put("type", value.getClass().getSimpleName());
            if (value instanceof AST) fields.put("origin", ((AST)value).getOrigin());
            for (Field field : value.getClass().getFields()) {
                if (!Modifier.isStatic(field.getModifiers())) fields.put(field.getName(), field.get(value));
            }
        }
        List<String> items = new ArrayList<>();
        for (Map.Entry<String, Object> entry : fields.entrySet()) items.add(quote(entry.getKey()) + ":" + json(entry.getValue()));
        return "{" + String.join(",", items) + "}";
    }
    private static String quote(String text) {
        StringBuilder result = new StringBuilder("\"");
        for (char c : text.toCharArray()) {
            switch (c) {
                case '\\': result.append("\\\\"); break;
                case '"': result.append("\\\""); break;
                case '\n': result.append("\\n"); break;
                case '\r': result.append("\\r"); break;
                case '\t': result.append("\\t"); break;
                default: if (c < 32) result.append(String.format("\\u%04x", (int)c)); else result.append(c);
            }
        }
        return result.append('"').toString();
    }
}

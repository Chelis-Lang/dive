// Development-only pinned translator adapter. Never linked into dive.
import java.nio.file.*;
import java.nio.charset.StandardCharsets;
import java.util.*;
import pcal.*;
import java.lang.reflect.*;

public class PlusCalProbe {
    public static void main(String[] args) throws Exception {
        boolean raw = args.length > 0 && args[0].equals("--source-grammar");
        for (String filename : Arrays.copyOfRange(args, raw ? 1 : 0, args.length)) {
            try {
                PcalParams.resetParams();
                String text = Files.readString(Path.of(filename));
                List<String> lines = Arrays.asList(text.split("\\r?\\n", -1));
                int line = -1, column = -1;
                boolean fair = false;
                for (int i = 0; i < lines.size() && line < 0; i++) {
                    int index = lines.get(i).indexOf("--fair");
                    if (index >= 0) { line = i; column = index + 6; fair = true; }
                    else { index = lines.get(i).indexOf("--algorithm");
                        if (index >= 0) { line = i; column = index + 11; }
                    }
                }
                if (line < 0) throw new IllegalArgumentException("no algorithm marker");
                TLAtoPCalMapping mapping = new TLAtoPCalMapping();
                mapping.algLine = line; mapping.algColumn = column;
                PcalParams.tlaPcalMapping = mapping;
                PcalCharReader reader = new PcalCharReader(new Vector<String>(lines), line, column,
                    lines.size(), lines.get(lines.size() - 1).codePointCount(0, lines.get(lines.size() - 1).length()));
                Object ast = raw ? grammar(reader, fair) : ParseAlgorithm.getAlgorithm(reader, fair);
                System.out.println(filename + "\taccepted\t" + encode(json(ast)));
            } catch (Throwable error) {
                System.out.println(filename + "\trejected\t" + encode(error.getClass().getName() + ": " + error.getMessage()));
            }
        }
    }
    private static String encode(String text) {
        return Base64.getEncoder().encodeToString(text.getBytes(StandardCharsets.UTF_8));
    }
    // Secondary adapter: invokes the original public parsing methods, stopping
    // before getAlgorithm's label insertion / global macro expansion / checkBody.
    // GetMacro itself expands earlier macro calls and GetStmt folds call tails.
    @SuppressWarnings({"rawtypes", "unchecked"})
    private static Object grammar(PcalCharReader reader, boolean fair) throws Throwable {
        ParseAlgorithm.Init(reader);
        if (fair) ParseAlgorithm.MustGobbleThis("algorithm");
        String name = ParseAlgorithm.GetAlgToken();
        boolean c = ParseAlgorithm.PeekAtAlgToken(1).equals("{");
        if (c) { ParseAlgorithm.cSyntax = true; ParseAlgorithm.MustGobbleThis("{"); }
        else ParseAlgorithm.pSyntax = true;
        Map<String, Object> result = new TreeMap<>();
        result.put("name", name); result.put("syntax", c ? "C" : "P"); result.put("fair", fair);
        String token = ParseAlgorithm.PeekAtAlgToken(1);
        result.put("decls", token.equals("variable") || token.equals("variables") ? ParseAlgorithm.GetVarDecls() : new Vector());
        if (ParseAlgorithm.PeekAtAlgToken(1).equals("define")) {
            ParseAlgorithm.MustGobbleThis("define");
            if (c) ParseAlgorithm.GobbleThis("{");
            result.put("defs", ParseAlgorithm.GetDefineExpr());
            ParseAlgorithm.GobbleEndOrRightBrace("define");
            ParseAlgorithm.GobbleThis(";");
        }
        Vector macros = new Vector();
        while (ParseAlgorithm.PeekAtAlgToken(1).equals("macro")) macros.add(ParseAlgorithm.GetMacro(macros));
        result.put("macros", macros);
        Vector procedures = new Vector();
        while (ParseAlgorithm.PeekAtAlgToken(1).equals("procedure")) procedures.add(ParseAlgorithm.GetProcedure());
        result.put("prcds", procedures);
        token = ParseAlgorithm.PeekAtAlgToken(1);
        if (token.equals("fair") || token.equals("process")) {
            Vector processes = new Vector();
            while ((token = ParseAlgorithm.PeekAtAlgToken(1)).equals("fair") || token.equals("process")) {
                int fairness = AST.UNFAIR_PROC;
                if (token.equals("fair")) {
                    ParseAlgorithm.MustGobbleThis("fair"); fairness = AST.WF_PROC;
                    if (ParseAlgorithm.PeekAtAlgToken(1).equals("+")) { ParseAlgorithm.MustGobbleThis("+"); fairness = AST.SF_PROC; }
                }
                AST.Process process = ParseAlgorithm.GetProcess(); process.fairness = fairness; processes.add(process);
            }
            result.put("procs", processes);
            ParseAlgorithm.GobbleEndOrRightBrace("algorithm");
        } else {
            ParseAlgorithm.GobbleBeginOrLeftBrace(); result.put("body", ParseAlgorithm.GetStmtSeq());
            if (c) ParseAlgorithm.GobbleThis("}");
            ParseAlgorithm.GobbleEndOrRightBrace("algorithm");
        }
        return result;
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

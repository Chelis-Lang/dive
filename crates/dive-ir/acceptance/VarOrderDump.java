/*
 * Records TLC's state-variable order (the variables' varLoc).
 *
 * Usage (one module per JVM, from the module's directory, as TLC runs):
 *   java -cp tla2tools.jar:<libraries>:<classes> VarOrderDump <id> <Module.tla> [<Model.cfg>]
 *
 * Prints one JSON line:
 *   {"id", "module", "via", "vars": [...], "keys": n}
 * or {"id", "error": ...} if SANY or TLC rejects the module.
 *
 * With a configuration it constructs TLC's FastTool as tlc2.TLC does ("via":
 * "tlc") and prints SpecProcessor.getVariablesNodes(), checking that each
 * variable's UniqueString.getVarLoc() is its index: the order of
 * TLCStateMut's value array, which TLCStateMut.fingerPrint walks. Without one it
 * runs SANY as SpecProcessor does ("via": "sany") and prints
 * ModuleNode.getVariableDecls() of the root module, which is what
 * SpecProcessor.processSpec assigns the varLocs from. "keys" is the number of
 * keys in the root module's context (its Hashtable), read by reflection; it
 * needs --add-opens java.base/java.util=ALL-UNNAMED and is -1 without it. It
 * reimplements nothing.
 */
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.util.HashMap;
import java.util.Hashtable;
import java.util.StringJoiner;

import tla2sany.drivers.SANY;
import tla2sany.modanalyzer.SpecObj;
import tla2sany.semantic.Context;
import tla2sany.semantic.ModuleNode;
import tla2sany.semantic.OpDeclNode;
import tlc2.tool.impl.FastTool;
import tlc2.tool.impl.Tool;
import tlc2.util.FP64;
import tlc2.value.RandomEnumerableValues;
import util.SimpleFilenameToStream;
import util.ToolIO;

public class VarOrderDump {
    static String q(String s) {
        if (s == null) return "null";
        StringBuilder b = new StringBuilder("\"");
        for (char c : s.toCharArray()) {
            switch (c) {
                case '"': b.append("\\\""); break;
                case '\\': b.append("\\\\"); break;
                case '\n': b.append("\\n"); break;
                default:
                    if (c < 0x20) b.append(String.format("\\u%04x", (int) c));
                    else b.append(c);
            }
        }
        return b.append('"').toString();
    }

    static int keys(ModuleNode root) {
        try {
            Context ctx = root.getContext();
            Field f = Context.class.getDeclaredField("table");
            f.setAccessible(true);
            return ((Hashtable<?, ?>) f.get(ctx)).size();
        } catch (Throwable t) {
            return -1;
        }
    }

    public static void main(String[] args) {
        String id = args[0];
        String module = args[1].replaceAll("\\.tla$", "");
        PrintStream out = System.out;
        System.setOut(System.err);
        ToolIO.setUserDir(System.getProperty("user.dir"));
        String result;
        try {
            OpDeclNode[] vars;
            ModuleNode root;
            String via;
            if (args.length > 2) {
                String cfg = args[2].replaceAll("\\.cfg$", "");
                FP64.Init(0);
                RandomEnumerableValues.setSeed(0);
                Tool tool = new FastTool(module, cfg, new SimpleFilenameToStream(), Tool.Mode.MC,
                        new HashMap<>());
                vars = tool.getSpecProcessor().getVariablesNodes();
                root = tool.getSpecProcessor().getRootModule();
                for (int i = 0; i < vars.length; i++) {
                    if (vars[i].getName().getVarLoc() != i) {
                        throw new IllegalStateException("varLoc of " + vars[i].getName() + " is "
                                + vars[i].getName().getVarLoc() + ", not " + i);
                    }
                }
                via = "tlc";
            } else {
                SpecObj spec = new SpecObj(module, new SimpleFilenameToStream());
                SANY.frontEndMain(spec, module, System.err);
                if (!spec.parseErrors.isSuccess() || !spec.semanticErrors.isSuccess()
                        || spec.getExternalModuleTable() == null
                        || spec.getExternalModuleTable().getRootModule() == null) {
                    throw new IllegalStateException("SANY rejects the module");
                }
                root = spec.getExternalModuleTable().getRootModule();
                vars = root.getVariableDecls();
                via = "sany";
            }
            StringJoiner names = new StringJoiner(",", "[", "]");
            for (OpDeclNode v : vars) names.add(q(v.getName().toString()));
            result = "{\"id\":" + q(id) + ",\"module\":" + q(args[1]) + ",\"via\":" + q(via) + ",\"vars\":"
                    + names + ",\"keys\":" + keys(root) + "}";
        } catch (Throwable t) {
            result = "{\"id\":" + q(id) + ",\"error\":" + q(t.getClass().getName() + ": " + t.getMessage()) + "}";
        }
        out.println(result);
        out.flush();
        System.exit(0);
    }
}

/*
 * Records how TLC binds a model: what tlc2.tool.impl.SpecProcessor and Spec
 * make of a specification and its configuration.
 *
 * Usage (one model per JVM, from the model's directory, as TLC runs):
 *   java -cp tla2tools.jar:<libraries>:<classes> ModelDump <id> <Module.tla> <Model.cfg> [dumptrace]
 *
 * With "dumptrace", the tool is built with the parameters `-dumpTrace json
 * <file>` gives it (ParameterizedSpecObj with the _JsonTrace post condition),
 * as the conformance harness runs TLC. Without it, as users run TLC.
 *
 * Prints one JSON line:
 *   {"id", "modules": [...], "rootConstants": [[name, value]...],
 *    "constantDefns": [[module, name, value]...], "inits": [...],
 *    "next": bool, "actions": [label...], "invariants": [...], "impliedInits": [...],
 *    "impliedActions": [...], "temporals": [...], "impliedTemporals": [...],
 *    "constraints": [...], "actionConstraints": [...], "view": ..., "alias": ...,
 *    "aliasSpec": location or {"error": code},
 *    "postconditions": [...] or {"error": code}, "symmetry": ..., "deadlock": bool,
 *    "interned": [...], "out": "..."}
 * or {"id", "error": {"code", "class", "message"}, "interned", "out"} if TLC
 * rejects the model while processing it. "out" is everything TLC printed
 * while processing (SANY's progress, Print output of constant definitions,
 * warnings). "interned" is util.UniqueString's table in token order.
 * It constructs TLC's FastTool as tlc2.TLC does and reimplements nothing.
 */
import java.io.ByteArrayOutputStream;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.StringJoiner;
import java.util.TreeMap;

import tla2sany.semantic.ExprNode;
import tla2sany.semantic.ModuleNode;
import tla2sany.semantic.OpDeclNode;
import tla2sany.semantic.OpDefNode;
import tla2sany.semantic.OpDefOrDeclNode;
import tla2sany.semantic.SemanticNode;
import tlc2.tool.Action;
import tlc2.tool.impl.FastTool;
import tlc2.tool.impl.ParameterizedSpecObj;
import tlc2.tool.impl.Tool;
import tlc2.util.FP64;
import tlc2.util.Vect;
import tlc2.value.RandomEnumerableValues;
import util.Assert;
import util.InternTable;
import util.SimpleFilenameToStream;
import util.ToolIO;
import util.UniqueString;

public class ModelDump {
    static String q(String s) {
        if (s == null) return "null";
        StringBuilder b = new StringBuilder("\"");
        for (char c : s.toCharArray()) {
            switch (c) {
                case '"': b.append("\\\""); break;
                case '\\': b.append("\\\\"); break;
                case '\n': b.append("\\n"); break;
                case '\r': b.append("\\r"); break;
                case '\t': b.append("\\t"); break;
                default:
                    if (c < 0x20) b.append(String.format("\\u%04x", (int) c));
                    else b.append(c);
            }
        }
        return b.append('"').toString();
    }

    static String names(String[] ns) {
        StringJoiner j = new StringJoiner(",", "[", "]");
        if (ns != null) for (String n : ns) j.add(q(n));
        return j.toString();
    }

    static String exprNames(ExprNode[] es, int toolId) {
        StringJoiner j = new StringJoiner(",", "[", "]");
        if (es != null) for (ExprNode e : es) {
            Object o = e.getToolObject(toolId);
            if (o instanceof OpDefNode) j.add(q(((OpDefNode) o).getName().toString()));
            else if (o instanceof Action && ((Action) o).getOpDef() != null) j.add(q(((Action) o).getOpDef().getName().toString()));
            else j.add(q(e.getLocation().toString()));
        }
        return j.toString();
    }

    static String interned() {
        try {
            Field f = InternTable.class.getDeclaredField("table");
            f.setAccessible(true);
            UniqueString[] table = (UniqueString[]) f.get(UniqueString.internTbl);
            TreeMap<Integer, String> byTok = new TreeMap<>();
            for (UniqueString u : table) if (u != null) byTok.put(u.getTok(), u.toString());
            StringJoiner j = new StringJoiner(",", "[", "]");
            int expect = -1;
            for (Map.Entry<Integer, String> e : byTok.entrySet()) {
                if (expect >= 0 && e.getKey() != expect) j.add("{\"gap\":" + e.getKey() + "}");
                j.add(q(e.getValue()));
                expect = e.getKey() + 1;
            }
            return "{\"first\":" + (byTok.isEmpty() ? 0 : byTok.firstKey()) + ",\"names\":" + j + "}";
        } catch (ReflectiveOperationException e) {
            return q("unavailable: " + e);
        }
    }

    public static void main(String[] args) {
        String id = args[0];
        String module = args[1].replaceAll("\\.tla$", "");
        String cfg = args[2].replaceAll("\\.cfg$", "");
        boolean dumpTrace = args.length > 3 && args[3].equals("dumptrace");
        PrintStream out = System.out;
        ByteArrayOutputStream buf = new ByteArrayOutputStream();
        PrintStream cap = new PrintStream(buf, true, StandardCharsets.UTF_8);
        System.setOut(cap);
        ToolIO.out = cap;
        ToolIO.setUserDir(System.getProperty("user.dir"));
        FP64.Init(0);
        RandomEnumerableValues.setSeed(0);
        Map<String, Object> params = new HashMap<>();
        if (dumpTrace) {
            List<ParameterizedSpecObj.PostCondition> pcs = new ArrayList<>();
            pcs.add(new ParameterizedSpecObj.PostCondition("_JsonTrace", "_JsonTrace", "_JsonTraceFile",
                    System.getProperty("java.io.tmpdir") + "/trace.json"));
            params.put(ParameterizedSpecObj.POST_CONDITIONS, pcs);
        }
        String result;
        try {
            Tool tool = new FastTool(module, cfg, new SimpleFilenameToStream(), Tool.Mode.MC, params);
            int tid = tool.getId();
            StringJoiner mods = new StringJoiner(",", "[", "]");
            for (ModuleNode m : tool.getSpecProcessor().getModuleTbl().getModuleNodes()) mods.add(q(m.getName().toString()));
            StringJoiner rc = new StringJoiner(",", "[", "]");
            for (OpDeclNode d : tool.getSpecProcessor().getRootModule().getConstantDecls()) {
                Object v = d.getToolObject(tid);
                String vs = v instanceof OpDefNode ? "<-" + ((OpDefNode) v).getName() : String.valueOf(v);
                rc.add("[" + q(d.getName().toString()) + "," + q(vs) + "]");
            }
            StringJoiner cd = new StringJoiner(",", "[", "]");
            TreeMap<String, String> sorted = new TreeMap<>();
            for (Map.Entry<ModuleNode, Map<OpDefOrDeclNode, Object>> e : tool.getSpecProcessor().getConstantDefns().entrySet())
                for (Map.Entry<OpDefOrDeclNode, Object> d : e.getValue().entrySet())
                    sorted.put(e.getKey().getName() + "!" + d.getKey().getName(), "[" + q(e.getKey().getName().toString()) + "," + q(d.getKey().getName().toString()) + "," + q(String.valueOf(d.getValue())) + "]");
            sorted.values().forEach(cd::add);
            StringJoiner inits = new StringJoiner(",", "[", "]");
            Vect<Action> iv = tool.getInitStateSpec();
            for (int i = 0; i < iv.size(); i++) inits.add(q(iv.elementAt(i).getLocation()));
            Action next = tool.getNextStateSpec();
            StringJoiner acts = new StringJoiner(",", "[", "]");
            if (next != null) for (Action a : tool.getActions()) acts.add(q(a.getLocation()));
            // TLC looks the post conditions and the alias up only after
            // checking or when it prints a trace; their errors are reported
            // apart.
            String pcs;
            try {
                StringJoiner pj = new StringJoiner(",", "[", "]");
                for (Action a : tool.getPostConditionSpecs()) pj.add(q(a.getNameOfDefault()));
                pcs = pj.toString();
            } catch (Assert.TLCRuntimeException e) {
                pcs = "{\"error\":" + e.errorCode + "}";
            }
            String aliasOut;
            try {
                aliasOut = tool.hasAlias() ? q(tool.getAliasSpec().getLocation().toString()) : "null";
            } catch (Assert.TLCRuntimeException e) {
                aliasOut = "{\"error\":" + e.errorCode + "}";
            }
            SemanticNode view = tool.getViewSpec();
            String alias = tool.getModelConfig().getAlias();
            result = "{\"id\":" + q(id) + ",\"dumpTrace\":" + dumpTrace + ",\"modules\":" + mods + ",\"rootConstants\":" + rc
                    + ",\"constantDefns\":" + cd + ",\"inits\":" + inits + ",\"next\":" + (next != null)
                    + ",\"actions\":" + acts
                    + ",\"invariants\":" + names(tool.getInvNames()) + ",\"impliedInits\":" + names(tool.getImpliedInitNames())
                    + ",\"impliedActions\":" + names(tool.getImpliedActNames()) + ",\"temporals\":" + names(tool.getTemporalNames())
                    + ",\"impliedTemporals\":" + names(tool.getImpliedTemporalNames())
                    + ",\"constraints\":" + exprNames(tool.getModelConstraints(), tid)
                    + ",\"actionConstraints\":" + exprNames(tool.getActionConstraints(), tid)
                    + ",\"view\":" + (view == null ? "null" : q(view.getLocation().toString()))
                    + ",\"alias\":" + q(alias) + ",\"aliasSpec\":" + aliasOut
                    + ",\"postconditions\":" + pcs
                    + ",\"symmetry\":" + q(tool.getModelConfig().getSymmetry())
                    + ",\"deadlock\":" + tool.getModelConfig().getCheckDeadlock()
                    + ",\"interned\":" + interned() + ",\"out\":" + q(buf.toString(StandardCharsets.UTF_8)) + "}";
        } catch (Throwable t) {
            int code = t instanceof Assert.TLCRuntimeException ? ((Assert.TLCRuntimeException) t).errorCode : -1;
            result = "{\"id\":" + q(id) + ",\"dumpTrace\":" + dumpTrace + ",\"error\":{\"code\":" + code + ",\"class\":"
                    + q(t.getClass().getName()) + ",\"message\":" + q(t.getMessage()) + "},\"interned\":" + interned()
                    + ",\"out\":" + q(buf.toString(StandardCharsets.UTF_8)) + "}";
        }
        out.println(result);
        out.flush();
        System.exit(0);
    }
}

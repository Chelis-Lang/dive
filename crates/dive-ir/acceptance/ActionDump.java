/*
 * Records how TLC splits a model's next-state relation into actions.
 *
 * Usage (one model per JVM, from the model's directory, as TLC runs):
 *   java -cp tla2tools.jar:<libraries>:<classes> ActionDump <id> <Module.tla> <Model.cfg>
 *
 * Prints one JSON line:
 *   {"id", "module", "next": {"name", "kind", "location"}, "overrides": {...}, "inits": [...], "actions": [...]}
 * or {"id", "error": ...} if TLC rejects the model while processing it.
 * Each action is TLC's own Action object: "name" (getName, or null when
 * unnamed), "label" (getLocation(): the string TLC prints in traces),
 * "location" (pred.getLocation()), "kind" (the predicate's node kind),
 * "params" (the action definition's parameters bound in its context) and
 * "context" (every binding of its context, innermost first, as TLC prints
 * values). The actions are tool.getActions(), the inits
 * tool.getInitStateSpec(), in TLC's order; "overrides" the configuration's
 * `<-` replacements of definitions (ModelConfig.getOverrides). It constructs TLC's FastTool as
 * tlc2.TLC does and reimplements nothing.
 */
import java.io.PrintStream;
import java.util.HashMap;
import java.util.StringJoiner;

import tla2sany.semantic.FormalParamNode;
import tla2sany.semantic.SemanticNode;
import tla2sany.st.Location;
import tlc2.tool.Action;
import tlc2.tool.impl.FastTool;
import tlc2.tool.impl.Tool;
import tlc2.util.Context;
import tlc2.util.FP64;
import tlc2.util.Vect;
import tlc2.value.RandomEnumerableValues;
import util.SimpleFilenameToStream;
import util.ToolIO;

public class ActionDump {
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

    static String loc(Location l) {
        return "{\"module\":" + q(l.source()) + ",\"beginLine\":" + l.beginLine() + ",\"beginColumn\":"
                + l.beginColumn() + ",\"endLine\":" + l.endLine() + ",\"endColumn\":" + l.endColumn()
                + ",\"text\":" + q(l.toString()) + "}";
    }

    static String kind(SemanticNode n) {
        return n.getClass().getSimpleName();
    }

    static String action(Action a) {
        StringJoiner params = new StringJoiner(",", "[", "]");
        if (a.getOpDef() != null) {
            for (FormalParamNode p : a.getOpDef().getParams()) {
                if (a.con.lookup(p) != null) params.add(q(p.getName().toString()));
            }
        }
        StringJoiner ctx = new StringJoiner(",", "[", "]");
        Context c = a.con;
        while (c != null && !c.isEmpty()) {
            if (c.getName() != null) {
                Object v = c.getValue();
                ctx.add("[" + q(c.getName().getName().toString()) + "," + q(String.valueOf(v)) + "]");
            }
            c = c.next();
        }
        return "{\"name\":" + (a.isNamed() ? q(a.getName().toString()) : "null") + ",\"label\":"
                + q(a.getLocation()) + ",\"location\":" + loc(a.pred.getLocation()) + ",\"kind\":"
                + q(kind(a.pred)) + ",\"params\":" + params + ",\"context\":" + ctx + "}";
    }

    public static void main(String[] args) {
        String id = args[0];
        String module = args[1].replaceAll("\\.tla$", "");
        String cfg = args[2].replaceAll("\\.cfg$", "");
        // SANY and TLC print their progress on ToolIO.out; keep stdout for
        // the result.
        PrintStream out = System.out;
        System.setOut(System.err);
        ToolIO.setUserDir(System.getProperty("user.dir"));
        FP64.Init(0);
        RandomEnumerableValues.setSeed(0);
        String result;
        try {
            Tool tool = new FastTool(module, cfg, new SimpleFilenameToStream(), Tool.Mode.MC,
                    new HashMap<>());
            Action next = tool.getNextStateSpec();
            String nextJson = next == null ? "null"
                    : "{\"name\":" + (next.getOpDef() == null ? "null" : q(next.getOpDef().getName().toString()))
                            + ",\"kind\":" + q(kind(next.pred)) + ",\"location\":" + loc(next.pred.getLocation())
                            + "}";
            StringJoiner overrides = new StringJoiner(",", "{", "}");
            java.util.Hashtable<String, String> ov = tool.getModelConfig().getOverrides();
            java.util.List<String> keys = new java.util.ArrayList<>(ov.keySet());
            java.util.Collections.sort(keys);
            for (String k : keys) overrides.add(q(k) + ":" + q(ov.get(k)));
            StringJoiner inits = new StringJoiner(",", "[", "]");
            Vect<Action> iv = tool.getInitStateSpec();
            for (int i = 0; i < iv.size(); i++) inits.add(action(iv.elementAt(i)));
            StringJoiner acts = new StringJoiner(",", "[", "]");
            for (Action a : tool.getActions()) acts.add(action(a));
            result = "{\"id\":" + q(id) + ",\"module\":" + q(args[1]) + ",\"next\":" + nextJson + ",\"overrides\":" + overrides + ",\"inits\":" + inits + ",\"actions\":" + acts
                    + "}";
        } catch (Throwable t) {
            result = "{\"id\":" + q(id) + ",\"error\":" + q(t.getClass().getName() + ": " + t.getMessage()) + "}";
        }
        out.println(result);
        out.flush();
        System.exit(0);
    }
}

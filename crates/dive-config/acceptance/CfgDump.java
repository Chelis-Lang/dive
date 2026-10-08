/*
 * Records TLC's own interpretation of model configuration (.cfg) files.
 *
 * Usage: java -cp tla2tools.jar:<classes> CfgDump <root> <relative-path>...
 *
 * For each file, prints one JSON object per line:
 *   {"path": ..., "cfg": <file text> | "cfg_base64": ..., "tlc": {...}}
 * where "tlc" is what tlc2.tool.impl.ModelConfig.parse() produced, in the
 * canonical shape dive-config's tests compare against (empty fields omitted).
 * It calls TLC's parser directly; nothing here re-implements it.
 */
import java.lang.reflect.Field;
import java.nio.ByteBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.*;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import tlc2.tool.impl.ModelConfig;
import tlc2.util.Vect;
import tlc2.value.impl.*;
import util.Assert.TLCRuntimeException;

public class CfgDump {
    static String q(String s) {
        StringBuilder b = new StringBuilder("\"");
        for (char c : s.toCharArray()) {
            switch (c) {
                case '"': b.append("\\\""); break;
                case '\\': b.append("\\\\"); break;
                case '\n': b.append("\\n"); break;
                case '\r': b.append("\\r"); break;
                case '\t': b.append("\\t"); break;
                default:
                    if (c < 0x20 || c == 0x2028 || c == 0x2029 || (c >= 0x7f && c < 0xa0)) {
                        b.append(String.format("\\u%04x", (int) c));
                    } else {
                        b.append(c);
                    }
            }
        }
        return b.append('"').toString();
    }

    static String val(Object o) {
        if (o instanceof IntValue) return "{\"int\":" + ((IntValue) o).val + "}";
        if (o instanceof StringValue) return "{\"string\":" + q(((StringValue) o).val.toString()) + "}";
        if (o == BoolValue.ValTrue) return "{\"bool\":true}";
        if (o == BoolValue.ValFalse) return "{\"bool\":false}";
        if (o instanceof ModelValue) return "{\"model\":" + q(((ModelValue) o).val.toString()) + "}";
        if (o instanceof SetEnumValue) {
            ValueVec v = ((SetEnumValue) o).elems;
            StringBuilder b = new StringBuilder("{\"set\":[");
            for (int i = 0; i < v.size(); i++) { if (i > 0) b.append(','); b.append(val(v.elementAt(i))); }
            return b.append("]}").toString();
        }
        throw new IllegalStateException("unexpected value " + o.getClass());
    }

    static String names(Vect v) {
        StringBuilder b = new StringBuilder("[");
        for (int i = 0; i < v.size(); i++) { if (i > 0) b.append(','); b.append(q((String) v.elementAt(i))); }
        return b.append(']').toString();
    }

    /** A constant line: [lhs, args..., value]. */
    static String line(Vect l) {
        StringBuilder b = new StringBuilder("{\"lhs\":" + q((String) l.elementAt(0)));
        if (l.size() > 2) {
            b.append(",\"args\":[");
            for (int i = 1; i < l.size() - 1; i++) { if (i > 1) b.append(','); b.append(val(l.elementAt(i))); }
            b.append(']');
        }
        return b.append(",\"value\":").append(val(l.elementAt(l.size() - 1))).append('}').toString();
    }

    static String lines(Vect v) {
        StringBuilder b = new StringBuilder("[");
        for (int i = 0; i < v.size(); i++) { if (i > 0) b.append(','); b.append(line((Vect) v.elementAt(i))); }
        return b.append(']').toString();
    }

    static String strMap(Map<?, ?> m) {
        TreeMap<String, String> t = new TreeMap<>();
        for (Map.Entry<?, ?> e : m.entrySet()) t.put((String) e.getKey(), (String) e.getValue());
        StringBuilder b = new StringBuilder("{");
        for (Map.Entry<String, String> e : t.entrySet()) {
            if (b.length() > 1) b.append(',');
            b.append(q(e.getKey())).append(':').append(q(e.getValue()));
        }
        return b.append('}').toString();
    }

    static void field(StringBuilder out, String key, String s) {
        if (!s.isEmpty()) out.append(',').append(q(key)).append(':').append(q(s));
    }

    static void list(StringBuilder out, String key, Vect v) {
        if (v.size() > 0) out.append(',').append(q(key)).append(':').append(names(v));
    }

    static final Pattern LEX_LINE = Pattern.compile("^Lexical error at line (\\d+), column (\\d+)\\.");

    static String interpret(Path file) {
        StringBuilder out = new StringBuilder("{");
        ModelConfig mc = new ModelConfig(file.toAbsolutePath().toString(), null);
        try {
            mc.parse();
            out.append("\"ok\":true");
            field(out, "init", mc.getInit());
            field(out, "next", mc.getNext());
            field(out, "specification", mc.getSpec());
            field(out, "view", mc.getView());
            field(out, "symmetry", mc.getSymmetry());
            field(out, "alias", mc.getAlias());
            field(out, "periodic", mc.getPeriodic());
            field(out, "rl_reward", mc.getRLReward());
            Field f = ModelConfig.class.getDeclaredField("configTbl");
            f.setAccessible(true);
            Object dl = ((Hashtable) f.get(mc)).get(ModelConfig.CheckDeadlock);
            if (dl instanceof Boolean) out.append(",\"check_deadlock\":").append(dl);
            list(out, "invariants", mc.getInvariants());
            list(out, "properties", mc.getProperties());
            list(out, "constraints", mc.getConstraints());
            list(out, "action_constraints", mc.getActionConstraints());
            list(out, "postconditions", mc.getPostConditions());
            list(out, "possible", mc.getPossible());
            if (mc.getConstants().size() > 0) out.append(",\"constants\":").append(lines(mc.getConstants()));
            if (!mc.getModConstants().isEmpty()) {
                TreeMap<String, String> t = new TreeMap<>();
                for (Object e : mc.getModConstants().entrySet()) {
                    Map.Entry me = (Map.Entry) e;
                    t.put((String) me.getKey(), lines((Vect) me.getValue()));
                }
                out.append(",\"module_constants\":{");
                boolean first = true;
                for (Map.Entry<String, String> e : t.entrySet()) {
                    if (!first) out.append(',');
                    first = false;
                    out.append(q(e.getKey())).append(':').append(e.getValue());
                }
                out.append('}');
            }
            if (!mc.getOverrides().isEmpty()) out.append(",\"overrides\":").append(strMap(mc.getOverrides()));
            if (!mc.getModOverrides().isEmpty()) {
                TreeMap<String, String> t = new TreeMap<>();
                for (Object e : mc.getModOverrides().entrySet()) {
                    Map.Entry me = (Map.Entry) e;
                    t.put((String) me.getKey(), strMap((Map) me.getValue()));
                }
                out.append(",\"module_overrides\":{");
                boolean first = true;
                for (Map.Entry<String, String> e : t.entrySet()) {
                    if (!first) out.append(',');
                    first = false;
                    out.append(q(e.getKey())).append(':').append(e.getValue());
                }
                out.append('}');
            }
            ModelValue.setValues();
            ModelValue[] mvs = ModelValue.mvs.clone();
            Arrays.sort(mvs, Comparator.comparingInt(m -> m.index));
            if (mvs.length > 0) {
                out.append(",\"model_values\":[");
                for (int i = 0; i < mvs.length; i++) { if (i > 0) out.append(','); out.append(q(mvs[i].val.toString())); }
                out.append(']');
            }
        } catch (TLCRuntimeException e) {
            out.append("\"ok\":false,\"error_code\":").append(e.errorCode);
            String[] p = e.parameters;
            if (e.errorCode == 5007 && p != null && p.length > 0) {
                Matcher m = LEX_LINE.matcher(p[0]);
                if (m.find()) out.append(",\"line\":").append(m.group(1)).append(",\"column\":").append(m.group(2));
            } else if (p != null && p.length > 0) {
                out.append(",\"line\":").append(p[0]);
                if (p.length > 1) out.append(",\"detail\":").append(q(p[1]));
            }
        } catch (Throwable e) {
            // TLC reports any other exception as EC.GENERAL (1000).
            out.append("\"ok\":false,\"error_code\":1000,\"exception\":").append(q(e.getClass().getName()));
        }
        return out.append('}').toString();
    }

    public static void main(String[] args) throws Exception {
        Path root = Path.of(args[0]);
        for (int i = 1; i < args.length; i++) {
            Path file = root.resolve(args[i]);
            byte[] bytes = Files.readAllBytes(file);
            String text;
            try {
                text = StandardCharsets.UTF_8.newDecoder()
                        .onMalformedInput(CodingErrorAction.REPORT)
                        .onUnmappableCharacter(CodingErrorAction.REPORT)
                        .decode(ByteBuffer.wrap(bytes)).toString();
            } catch (CharacterCodingException e) {
                text = null;
            }
            StringBuilder out = new StringBuilder("{\"path\":" + q(args[i]) + ",");
            if (text != null) {
                out.append("\"cfg\":").append(q(text));
            } else {
                out.append("\"cfg_base64\":").append(q(Base64.getEncoder().encodeToString(bytes)));
            }
            out.append(",\"tlc\":").append(interpret(file)).append('}');
            System.out.println(out);
        }
    }
}

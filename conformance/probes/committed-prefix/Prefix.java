import tlc2.tool.impl.PrefixSignals;
import tlc2.value.impl.*;
/** Deliberate native Next instrumentation; never a pure implementation claim. */
public final class Prefix {
 public static Value Gate(Value v)throws Exception{PrefixSignals.gate(((IntValue)v).val);return BoolValue.ValTrue;}
}

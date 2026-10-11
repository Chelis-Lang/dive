// Development-only configured reference validator; never used by native runtime.
package pcal;
import java.nio.file.*;
import java.nio.charset.StandardCharsets;
import java.lang.reflect.Method;
import java.util.*;
public final class OptionValidatorProbe {
 public static void main(String[] args) throws Exception {
  PcalParams.resetParams(); AST.indentDepth=0;
  if (args.length == 0 || trans.parseAndProcessArguments(args) != trans.STATUS_OK)
   throw new IllegalArgumentException("reference option parsing failed");
  List<String> input=Files.readAllLines(Path.of(args[args.length-1]),StandardCharsets.UTF_8);
  Method m=Validator.class.getDeclaredMethod("validate",List.class);m.setAccessible(true);
  System.out.println("VALIDATION\t"+m.invoke(null,input));
 }
}

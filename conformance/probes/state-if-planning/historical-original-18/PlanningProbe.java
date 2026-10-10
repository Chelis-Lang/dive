package tlc2.tool.liveness;

import java.util.HashMap;
import tlc2.tool.impl.FastTool;
import tlc2.tool.impl.Tool;
import tlc2.tool.EvalException;
import tlc2.util.FP64;
import tlc2.value.RandomEnumerableValues;
import util.SimpleFilenameToStream;
import util.ToolIO;

/** Actual pinned processLiveness admission, before any state graph exists. */
public final class PlanningProbe {
 public static void main(String[] args) throws Exception {
  java.io.PrintStream out = System.out;
  System.setOut(System.err); ToolIO.out = System.err;
  ToolIO.setUserDir(System.getProperty("user.dir"));
  FP64.Init(0); RandomEnumerableValues.setSeed(0);
  String phase = "binding";
  try {
  Tool tool = new FastTool(args[0], args[0], new SimpleFilenameToStream(),
                          Tool.Mode.MC, new HashMap<>());
  System.setOut(out);
  phase = "planning";
   OrderOfSolution[] groups = Liveness.processLiveness(tool, true);
   out.println("completion\tplanned");
   out.println("groups\t" + groups.length);
   for (int i = 0; i < groups.length; i++) {
    out.println("g" + i + ".state\t" + groups[i].getCheckState().length);
    out.println("g" + i + ".action\t" + groups[i].getCheckAction().length);
    out.println("g" + i + ".tableau\t" + groups[i].hasTableau());
   }
  } catch (EvalException e) {
   out.println("phase\t" + phase);
   out.println("completion\tevaluation-error");
   out.println("error_code\t" + e.getErrorCode());
   out.println("message\t" + e.getMessage().replace('\n', ' '));
  } catch (util.Assert.TLCRuntimeException e) {
   out.println("phase\t" + phase);
   out.println("completion\ttlc-error");
   out.println("error_code\t" + e.errorCode);
   out.println("message\t" + e.getMessage().replace('\n', ' '));
  }
 }
}

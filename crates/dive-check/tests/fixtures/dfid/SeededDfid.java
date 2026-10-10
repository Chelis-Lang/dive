// Secondary reference only: pinned DFID with an injected worker seed.
import java.lang.reflect.Field;
import java.util.Map;
import tlc2.TLCGlobals;
import tlc2.tool.*;
import tlc2.tool.impl.*;
import tlc2.util.*;
import util.*;
public final class SeededDfid extends DFIDModelChecker {
  static long seed;
  SeededDfid(ITool tool, String dir) throws Exception {
    super(tool, dir, new NoopStateWriter(), true, null, System.currentTimeMillis());
  }
  @Override protected IWorker[] startWorkers(AbstractChecker checker, int level) {
    try {
      DFIDWorker worker = new DFIDWorker(0, level, checker);
      Field field = DFIDWorker.class.getDeclaredField("rng");
      field.setAccessible(true);
      ((RandomGenerator) field.get(worker)).setSeed(seed);
      workers[0] = worker;
      System.out.println("SECONDARY_WORKER_SEED " + level + " " + seed);
      worker.start();
      return workers;
    } catch (ReflectiveOperationException e) { throw new IllegalStateException(e); }
  }
  public static void main(String[] args) throws Exception {
    seed = Long.parseLong(args[2]);
    TLCGlobals.DFIDMax = Integer.parseInt(args[1]);
    TLCGlobals.setNumWorkers(1);
    TLCGlobals.tool = true;
    ToolIO.setUserDir(System.getProperty("user.dir"));
    FP64.Init(0);
    tlc2.value.RandomEnumerableValues.setSeed(7);
    Tool tool = new FastTool(args[0], args[0], new SimpleFilenameToStream(), Tool.Mode.MC, Map.of());
    int code = new SeededDfid(tool, args[3]).modelCheck();
    System.out.println("SECONDARY_DECIDING_CODE " + code);
  }
}

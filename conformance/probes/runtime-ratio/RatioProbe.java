import java.lang.reflect.*;
import java.util.*;
import java.util.concurrent.*;
import tlc2.TLCGlobals;
import tlc2.tool.*;
import tlc2.tool.impl.*;
import tlc2.tool.fp.FPSet;
import tlc2.util.*;
import util.*;
/** Calls the unchanged pinned updater; clock samples bracket its real clock read. */
public final class RatioProbe {
 static final class Observed extends ModelChecker {
  Observed(ITool tool,String directory)throws Exception {
   super(tool,directory,new NoopStateWriter(),false,null,
    CompletableFuture.completedFuture((FPSet)null),0L);
  }
  void apply(long delta){super.updateRuntimeRatio(delta);}
 }
 public static void main(String[] args)throws Exception {
  ToolIO.setUserDir(System.getProperty("user.dir"));ToolIO.out=System.err;
  FP64.Init(0);TLCGlobals.setNumWorkers(1);
  java.io.PrintStream out=System.out;System.setOut(System.err);
  Tool tool=new FastTool("Ratio","Ratio",new SimpleFilenameToStream(),Tool.Mode.MC,new HashMap<>());
  Observed checker=new Observed(tool,args[0]);System.setOut(out);
  Field start=AbstractChecker.class.getDeclaredField("startTime");start.setAccessible(true);
  Field ratio=ModelChecker.class.getDeclaredField("runtimeRatio");ratio.setAccessible(true);
  double[] previous={0d,-0d,0.2d,0.999d,Double.NaN,Double.POSITIVE_INFINITY,Double.NEGATIVE_INFINITY};
  long[] elapsed={-120000L,0L,1L,60000L,120000L,Long.MAX_VALUE,Long.MIN_VALUE};
  long[] deltas={0L,1L,60000L,Long.MAX_VALUE};
  int id=0;
  for(double old:previous)for(long age:elapsed)for(long delta:deltas){
   long initial=System.currentTimeMillis()-age;
   start.setLong(checker,initial);ratio.setDouble(checker,old);
   long before=System.currentTimeMillis();checker.apply(delta);long after=System.currentTimeMillis();
   System.out.println("ROW\t"+(id++)+"\t"+Long.toHexString(Double.doubleToLongBits(old))+"\t"+initial+"\t"+TLCGlobals.progressInterval+"\t"+delta+"\t"+before+"\t"+after+"\t"+Long.toHexString(Double.doubleToLongBits(checker.getRuntimeRatio())));
  }
  for(long delta:new long[]{-1L,-60000L,Long.MIN_VALUE}){
   boolean failed=false;try{checker.apply(delta);}catch(AssertionError e){failed=true;}
   System.out.println("NEGATIVE\t"+delta+"\t"+failed);
  }
  System.exit(0);
 }
}

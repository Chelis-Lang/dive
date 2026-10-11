import java.lang.reflect.*;
import java.util.*;
import java.util.concurrent.*;
import tlc2.TLCGlobals;
import tlc2.tool.*;
import tlc2.tool.impl.*;
import tlc2.tool.fp.FPSet;
import tlc2.tool.queue.IStateQueue;
import tlc2.tool.liveness.ILiveCheck;
import tlc2.util.*;
import util.*;

/** Actual pinned doPeriodicWork with observed queue/live/checkpoint boundaries.
 * Callbacks supply outcomes; this is not a full ordinary model-checking run.
 */
public final class HookProbe {
 static void event(String s) { System.out.println("EVENT\t"+s); }
 static Object empty(Class<?> t) {
  if(t==boolean.class)return false;if(t==int.class)return 0;if(t==long.class)return 0L;
  if(t==double.class)return 0d;return null;
 }
 static class Observed extends ModelChecker {
  Observed(ITool tool,String directory)throws Exception {
   super(tool,directory,new NoopStateWriter(),false,null,
    CompletableFuture.completedFuture((FPSet)null),System.currentTimeMillis());
  }
  @Override protected void checkpoint(){event("checkpoint");}
  @Override protected void updateRuntimeRatio(long delta){event("ratio-update:"+delta);}
  void replaceLiveCheck(ILiveCheck live)throws Exception {
   Field f=AbstractChecker.class.getDeclaredField("liveCheck");f.setAccessible(true);
   ((ILiveCheck)f.get(this)).close();f.set(this,live);
  }
 }
 public static void main(String[] a)throws Exception {
  try { mainChecked(a); } catch(Throwable t) {
   System.setOut(new java.io.PrintStream(new java.io.FileOutputStream(java.io.FileDescriptor.out)));
   event("exception-class:"+t.getClass().getName());
   int code=t instanceof EvalException ? ((EvalException)t).getErrorCode() : t instanceof util.Assert.TLCRuntimeException ? ((util.Assert.TLCRuntimeException)t).errorCode : -1;
   event("exception-code:"+code);
   System.out.println("MESSAGE-BEGIN");System.out.println(t.getMessage());System.out.println("MESSAGE-END");t.printStackTrace(System.err);
   System.exit(2);
  }
 }
 public static void mainChecked(String[] a)throws Exception {
  ToolIO.setUserDir(System.getProperty("user.dir"));ToolIO.out=System.err;
  FP64.Init(0);TLCGlobals.setNumWorkers(1);TLCGlobals.chkptDuration=0;
  java.io.PrintStream actualOut=System.out;System.setOut(System.err);
  event("setup-start");
  Tool tool=new FastTool("Periodic","Periodic",new SimpleFilenameToStream(),Tool.Mode.MC,new HashMap<>());
  Observed checker=new Observed(tool,a[0]);
  System.setOut(actualOut);ToolIO.out=System.out;
  event("setup-complete");
  final boolean grow=Boolean.parseBoolean(a[2]), suspended=Boolean.parseBoolean(a[5]);
  final int checkCode=Integer.parseInt(a[6]);
  checker.theStateQueue=(IStateQueue)Proxy.newProxyInstance(HookProbe.class.getClassLoader(),new Class<?>[]{IStateQueue.class},(proxy,m,args)->{
   if(m.getName().equals("suspendAll")){event("suspend");return suspended;}
   if(m.getName().equals("resumeAll")){event("resume");return null;}
   return empty(m.getReturnType());
  });
  ILiveCheck live=(ILiveCheck)Proxy.newProxyInstance(HookProbe.class.getClassLoader(),new Class<?>[]{ILiveCheck.class},(proxy,m,args)->{
   if(m.getName().equals("doLiveCheck")){event("growth");return grow;}
   if(m.getName().equals("check")){event("check-force:"+args[1]);return checkCode;}
   return empty(m.getReturnType());
  });
  checker.replaceLiveCheck(live);
  Field ratio=ModelChecker.class.getDeclaredField("runtimeRatio");ratio.setAccessible(true);ratio.setDouble(checker,Double.parseDouble(a[1]));
  if(Boolean.parseBoolean(a[3]))checker.forceLiveCheck();
  if(Boolean.parseBoolean(a[4]))TLCGlobals.forceChkpt();
  event("start");int result=checker.doPeriodicWork();event("result:"+result);
  Field force=ModelChecker.class.getDeclaredField("forceLiveCheck");force.setAccessible(true);event("force-retained:"+force.getBoolean(checker));
  System.exit(0);
 }
}

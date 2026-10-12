import java.lang.reflect.*;
import java.nio.file.*;
import java.util.*;
import java.util.concurrent.*;
import tlc2.TLCGlobals;
import tlc2.tool.*;
import tlc2.tool.impl.*;
import tlc2.tool.fp.*;
import tlc2.tool.queue.*;
import tlc2.util.*;
import util.*;
/** Actual controller and actual queue; the helper dequeues only and is not TLCWorker. */
public final class OptionalServiceProbe {
 static void event(String x){System.out.println("EVENT\t"+x);}
 static class Observed extends ModelChecker {
  Observed(ITool tool,String dir)throws Exception{super(tool,dir,new NoopStateWriter(),false,null,new FPSetConfiguration(),System.currentTimeMillis());}
  protected void updateRuntimeRatio(long delta){event("ratio-update:"+delta);super.updateRuntimeRatio(delta);}
 }
 public static void main(String[] a)throws Throwable{
  java.io.PrintStream actual=System.out;Observed c=null;Thread helper=null;
  try{
   ToolIO.setUserDir(System.getProperty("user.dir"));ToolIO.out=System.err;System.setOut(System.err);
   FP64.Init(0);TLCGlobals.setNumWorkers(1);TLCGlobals.chkptDuration=0;
   Tool t=new FastTool("Snapshot","Snapshot",new SimpleFilenameToStream(),Tool.Mode.MC,new HashMap<>());
   c=new Observed(t,Files.createDirectories(Path.of("meta")).toString());
   System.setOut(actual);ToolIO.out=actual;
   event("configured-name:"+t.getModelConfig().getPeriodic());event("body-present:"+(t.getSpecProcessor().getPeriodic()!=null));
   event("assumptions:"+c.checkAssumptions());int init=c.doInit(false);event("init-result:"+init);event("queue-before:"+c.theStateQueue.size());
   if(init!=0){event("failed-init-no-service");System.exit(0);}
   boolean force=Boolean.parseBoolean(a[0]);if(force)c.forceLiveCheck();event("force-requested:"+force);
   final Observed checker=c;final Throwable[] fail={null};final int[] result={-999};
   Thread service=new Thread(()->{try{event("service-enter");result[0]=checker.doPeriodicWork();event("service-result:"+result[0]);}catch(Throwable e){fail[0]=e;event("service-exception:"+e.getClass().getName());event("service-exception-code:"+(e instanceof EvalException?((EvalException)e).getErrorCode():e instanceof Assert.TLCRuntimeException?((Assert.TLCRuntimeException)e).errorCode:-1));}},"actual-controller");
   Field stop=StateQueue.class.getDeclaredField("stop");stop.setAccessible(true);service.start();
   long deadline=System.nanoTime()+TimeUnit.SECONDS.toNanos(5);boolean paused=false;
   while(service.isAlive()&&System.nanoTime()<deadline){synchronized(c.theStateQueue){paused=stop.getBoolean(c.theStateQueue);}if(paused)break;Thread.sleep(1);}
   event("actual-stop-observed:"+paused);
   if(paused){helper=new Thread(()->{event("queue-helper-enter");TLCState value=checker.theStateQueue.sDequeue();event("queue-helper-return:"+(value==null?"EOF":value.lookup("x")));},"queue-only-helper");helper.start();}
   service.join(5000);if(service.isAlive())throw new AssertionError("service timeout");
   Field f=ModelChecker.class.getDeclaredField("forceLiveCheck");f.setAccessible(true);event("force-retained:"+f.getBoolean(c));
   synchronized(c.theStateQueue){event("stop-after-service:"+stop.getBoolean(c.theStateQueue));}event("queue-at-service-return:"+c.theStateQueue.size());
   event("observer-cleanup-start");c.theStateQueue.finishAll();if(helper!=null){helper.join(5000);if(helper.isAlive())throw new AssertionError("helper cleanup timeout");}event("observer-cleanup-complete");
   System.exit(fail[0]==null?0:2);
  }catch(Throwable e){System.setOut(actual);event("exception-class:"+e.getClass().getName());event("exception-code:"+(e instanceof EvalException?((EvalException)e).getErrorCode():e instanceof Assert.TLCRuntimeException?((Assert.TLCRuntimeException)e).errorCode:-1));e.printStackTrace(System.err);if(c!=null)c.theStateQueue.finishAll();System.exit(2);}
 }
}

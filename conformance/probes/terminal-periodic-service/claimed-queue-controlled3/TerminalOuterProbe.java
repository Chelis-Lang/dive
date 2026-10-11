import java.lang.reflect.*;
import java.nio.file.*;
import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicBoolean;
import tlc2.TLCGlobals;
import tlc2.tool.*;
import tlc2.tool.impl.*;
import tlc2.tool.fp.*;
import tlc2.tool.queue.*;
import tlc2.tool.liveness.*;
import tlc2.util.*;
import tlc2.value.IValue;
import tla2sany.semantic.SemanticNode;
import util.*;
/** Selected actual full modelCheck with transparent observers of real queue/live operations. */
public final class TerminalOuterProbe {
 static final CountDownLatch release=new CountDownLatch(1);
 static final AtomicBoolean held=new AtomicBoolean(false);
 static synchronized void event(String s){System.out.println("EVENT\t"+s);}
 static class Checker extends ModelChecker {
  Checker(ITool t,String p)throws Exception {super(t,p,new NoopStateWriter(),false,null,new FPSetConfiguration(),System.currentTimeMillis());}
  protected IWorker[] startWorkers(AbstractChecker c,int d){event("start-workers-enter");IWorker[] w=super.startWorkers(c,d);event("start-workers-return:"+w.length);return w;}
  protected void updateRuntimeRatio(long d){event("ratio-update:"+d);super.updateRuntimeRatio(d);}
  void workersAtReturn(){for(int i=0;i<workers.length;i++)event("worker-at-return:"+i+":"+((Thread)workers[i]).isAlive());event("checker-done-at-return:"+done);event("checker-error-at-return:"+errorCode);}
  void observerCleanup()throws Exception{event("observer-cleanup-enter");release.countDown();theStateQueue.finishAll();for(IWorker w:workers){Thread t=(Thread)w;if(t.isAlive())t.join(5000);if(t.isAlive())throw new AssertionError("cleanup thread alive");}event("observer-cleanup-return");}
 }
 static Object invoke(Method m,Object actual,Object[] a)throws Throwable {try{return m.invoke(actual,a);}catch(InvocationTargetException e){throw e.getCause();}}
 public static void main(String[] a)throws Throwable {
  TLCGlobals.tool=true;
  TLCGlobals.setNumWorkers(Integer.parseInt(a[0]));TLCGlobals.chkptDuration=0;FP64.Init(0);ToolIO.setUserDir(System.getProperty("user.dir"));
  FastTool t=new FastTool("Terminal","Terminal",new SimpleFilenameToStream(),Tool.Mode.MC,new HashMap<>());Path meta=Files.createDirectories(Path.of("meta"));IStateQueue q=new DiskStateQueue(meta.toString());
  IStateQueue.Factory.sq=(IStateQueue)Proxy.newProxyInstance(IStateQueue.class.getClassLoader(),new Class[]{IStateQueue.class},(p,m,args)->{
   String n=m.getName();boolean observe=n.equals("suspendAll")||n.equals("resumeAll")||n.equals("finishAll");if(observe)event("queue-"+n+"-enter:"+Thread.currentThread().getName()+":size="+q.size());
   if(n.equals("suspendAll")&&Thread.currentThread().getName().equals("main")){event("controlled-release-after-actual-suspend-entry");release.countDown();}
   Object r=invoke(m,q,args);
   if(n.equals("sDequeue")&&r!=null&&held.compareAndSet(false,true)){
    event("controlled-held-actual-claim:queue-size="+q.size());
    if(!release.await(10,TimeUnit.SECONDS))throw new AssertionError("controlled claim gate timeout");
    event("controlled-return-actual-claim");
   }if(observe)event("queue-"+n+"-return:"+r+":size="+q.size());if(n.equals("sDequeue")&&r==null)event("queue-dequeue-EOF:"+Thread.currentThread().getName());return r;
  });
  Checker c=new Checker(t,meta.toString());IStateQueue.Factory.sq=null;
  Field f=AbstractChecker.class.getDeclaredField("liveCheck");f.setAccessible(true);ILiveCheck actual=(ILiveCheck)f.get(c);
  ILiveCheck observed=(ILiveCheck)Proxy.newProxyInstance(ILiveCheck.class.getClassLoader(),new Class[]{ILiveCheck.class},(p,m,args)->{
   String n=m.getName();boolean o=n.equals("check")||n.equals("finalCheck")||n.equals("doLiveCheck");if(o)event("live-"+n+"-enter");Object r=invoke(m,actual,args);if(o)event("live-"+n+"-return:"+r);return r;
  });f.set(c,observed);
  event("modelCheck-enter");try{c.modelCheck();event("modelCheck-return");}catch(Throwable e){event("modelCheck-throw:"+e.getClass().getName());e.printStackTrace(System.err);}c.workersAtReturn();c.observerCleanup();
 }
}

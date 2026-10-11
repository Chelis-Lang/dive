import java.lang.reflect.*;
import java.util.*;
import java.util.concurrent.*;
import java.nio.file.*;
import tlc2.*;
import tlc2.tool.*;
import tlc2.tool.impl.*;
import tlc2.tool.fp.*;
import tlc2.tool.queue.*;
import tlc2.util.*;
import util.*;

/** Real local Worker/runTLC/queue execution; request injection is controlled. */
public final class ProgressProbe {
 static synchronized void event(String s){System.out.println("EVENT\t"+s);}
 static class Observed extends ModelChecker {
  Observed(ITool t,String dir)throws Exception{super(t,dir,new NoopStateWriter(),false,null,new FPSetConfiguration(),System.currentTimeMillis());}
  protected IWorker[] startWorkers(AbstractChecker c,int depth){event("start-workers");return super.startWorkers(c,depth);}
  protected void runTLCContinueDoing(int count,int depth)throws Exception{event("progress-wait");super.runTLCContinueDoing(count,depth);}
  protected void updateRuntimeRatio(long delta){event("ratio-update:"+delta);super.updateRuntimeRatio(delta);}
  void releaseInitialWait(){synchronized(this){notifyAll();}}
  void closeWorkers()throws Exception{theStateQueue.finishAll();for(IWorker w:workers)((Thread)w).join(5000);for(IWorker w:workers)if(((Thread)w).isAlive())throw new AssertionError("unjoined cleanup worker");event("cleanup-joined");}
  int alive(){int n=0;for(IWorker w:workers)if(((Thread)w).isAlive())n++;return n;}
  int finalCheck()throws Exception{return liveCheck.finalCheck(tool.noDebug());}
 }
 static Object invoke(Object target,Method method,Object[] args)throws Throwable{try{return method.invoke(target,args);}catch(InvocationTargetException e){throw e.getCause();}}
 public static void run(String[] a)throws Throwable{
  int width=Integer.parseInt(a[0]);String scenario=a[1];
  ToolIO.setUserDir(System.getProperty("user.dir"));ToolIO.out=System.err;FP64.Init(0);
  TLCGlobals.setNumWorkers(width);TLCGlobals.chkptDuration=0;
  Tool real=new FastTool("Progress","Progress",new SimpleFilenameToStream(),Tool.Mode.MC,new HashMap<>());
  Progress.scenario=scenario;
  CountDownLatch expanding=Progress.expanding,release=Progress.release,suspend=new CountDownLatch(1);
  Observed checker=new Observed(real,Files.createDirectories(Path.of("meta")).toString());
  IStateQueue actual=checker.theStateQueue;
  checker.theStateQueue=(IStateQueue)Proxy.newProxyInstance(ProgressProbe.class.getClassLoader(),new Class<?>[]{IStateQueue.class},(p,m,args)->{
   String name=m.getName();
   if(name.equals("suspendAll")){event("suspend-enter");suspend.countDown();Object r=invoke(actual,m,args);event("suspend-return:"+r);return r;}
   if(name.equals("resumeAll")||name.equals("finishAll"))event(name);
   if(name.equals("sDequeue")){Object r=invoke(actual,m,args);event("dequeue:"+(r==null?"EOF":"state"));return r;}
   return invoke(actual,m,args);
  });
  event("assumptions:"+checker.checkAssumptions());int init=checker.doInit(false);event("init-result:"+init);
  if(init!=0){event("failed-init-no-start");System.exit(0);}
  Thread injector=null;
  if(!(scenario.equals("natural")||scenario.equals("natural-safe")||scenario.equals("final-eof"))){
   injector=new Thread(()->{try{
    if(!expanding.await(5,TimeUnit.SECONDS))throw new AssertionError("no expansion");
    checker.forceLiveCheck();event("controlled-request-in-flight");checker.releaseInitialWait();
    if(!suspend.await(5,TimeUnit.SECONDS))throw new AssertionError("no actual suspension");
    if(scenario.equals("finish-during")){event("controlled-finish-during");actual.finishAll();}
    release.countDown();
   }catch(Throwable t){event("injector-error:"+t);release.countDown();actual.finishAll();}},"request-injector");injector.start();
  }
  int code=checker.runTLC(Integer.MAX_VALUE);event("runTLC-result:"+code);event("alive-at-return:"+checker.alive());
  event("generated-at-return:"+checker.getStatesGenerated());event("queue-at-return:"+actual.size());
  if(injector!=null)injector.join();
  Field errorField=AbstractChecker.class.getDeclaredField("errorCode");errorField.setAccessible(true);
  Field stackField=AbstractChecker.class.getDeclaredField("keepCallStack");stackField.setAccessible(true);
  int error=(int)errorField.get(checker);boolean keep=(boolean)stackField.get(checker);
  event("error-code-at-return:"+error);event("keep-call-stack-at-return:"+keep);
  if(code==0&&error==0&&!keep&&!scenario.equals("finish-during")){event("actual-final-check:"+checker.finalCheck());}
  checker.closeWorkers();event("done");System.exit(0);
 }
 public static void main(String[] a)throws Throwable{
  try{run(a);}catch(Throwable t){event("exception-class:"+t.getClass().getName());
   int code=t instanceof EvalException?((EvalException)t).getErrorCode():t instanceof Assert.TLCRuntimeException?((Assert.TLCRuntimeException)t).errorCode:-1;
   event("exception-code:"+code);t.printStackTrace(System.err);System.exit(2);}
 }
}

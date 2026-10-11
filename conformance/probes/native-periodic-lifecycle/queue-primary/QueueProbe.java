import tlc2.TLCGlobals;
import tlc2.tool.TLCState;
import tlc2.tool.TLCStateMut;
import tlc2.tool.queue.DiskStateQueue;
import tlc2.tool.queue.StateQueue;
import tla2sany.semantic.OpDeclNode;
import java.nio.file.*;
import java.lang.reflect.*;
import java.util.*;
import java.util.concurrent.*;

public final class QueueProbe {
  static final Field STOP;
  static final Field WAITING;
  static { try { STOP=StateQueue.class.getDeclaredField("stop");STOP.setAccessible(true);
    WAITING=StateQueue.class.getDeclaredField("numWaiting");WAITING.setAccessible(true);
  } catch(Exception e) { throw new ExceptionInInitializerError(e); } }
  static void awaitStop(DiskStateQueue q) throws Exception {
    long end=System.nanoTime()+TimeUnit.SECONDS.toNanos(5);
    for(;;) { synchronized(q) { if(STOP.getBoolean(q)) return; }
      if(System.nanoTime()>end) throw new TimeoutException("actual suspend stop not observed");
      Thread.yield(); }
  }
  static void run(Path root,int width,int repeat,String scenario) throws Exception {
    TLCGlobals.setNumWorkers(width);
    Path dir=Files.createDirectory(root.resolve(scenario+"-"+width+"-"+repeat));
    DiskStateQueue q=new DiskStateQueue(dir.toString());
    for(int i=0;i<width;i++) q.sEnqueue(TLCState.Empty.createEmpty());
    if(scenario.equals("finished-before")) {
      q.finishAll(); boolean suspended=q.suspendAll();
      if(suspended || q.sDequeue()!=null || q.size()!=width) throw new AssertionError("finished-before");
      System.out.printf("{\"scenario\":\"%s\",\"width\":%d,\"repeat\":%d,\"suspended\":false,\"waiting\":0,\"states_taken\":0,\"size\":%d}%n",scenario,width,repeat,q.size());
      return;
    }
    ExecutorService pool=Executors.newFixedThreadPool(width+1);
    CountDownLatch ready=new CountDownLatch(width),release=new CountDownLatch(1);
    List<Future<TLCState>> workers=new ArrayList<>();
    try {
      for(int i=0;i<width;i++) workers.add(pool.submit(()->{ ready.countDown();release.await();return q.sDequeue(); }));
      if(!ready.await(5,TimeUnit.SECONDS)) throw new TimeoutException("workers not ready");
      Future<Boolean> controller=pool.submit(()->q.suspendAll());
      awaitStop(q);
      if(scenario.equals("finished-during")) q.finishAll();
      release.countDown();
      boolean suspended=controller.get(5,TimeUnit.SECONDS);
      int waiting;
      synchronized(q) { waiting=WAITING.getInt(q); }
      if(scenario.equals("resume")) {
        if(!suspended || waiting!=width || q.size()!=width) throw new AssertionError("suspension barrier");
        q.resumeAll();
      } else if(suspended) throw new AssertionError("finished suspend returned true");
      int taken=0;
      for(Future<TLCState> worker:workers) if(worker.get(5,TimeUnit.SECONDS)!=null) taken++;
      if(taken!=(scenario.equals("resume")?width:0)) throw new AssertionError("taken");
      System.out.printf("{\"scenario\":\"%s\",\"width\":%d,\"repeat\":%d,\"suspended\":%s,\"waiting\":%d,\"states_taken\":%d,\"size\":%d}%n",scenario,width,repeat,suspended,waiting,taken,q.size());
    } finally { q.finishAll();release.countDown();pool.shutdown();
      if(!pool.awaitTermination(5,TimeUnit.SECONDS)) throw new TimeoutException("worker cleanup"); }
  }
  public static void main(String[] args) throws Exception {
    TLCStateMut.setVariables(new OpDeclNode[0]);
    Path root=Paths.get(args[0]);Files.createDirectories(root);
    for(int width:new int[]{1,4,8}) for(int repeat=0;repeat<8;repeat++)
      for(String scenario:new String[]{"resume","finished-before","finished-during"}) run(root,width,repeat,scenario);
  }
}

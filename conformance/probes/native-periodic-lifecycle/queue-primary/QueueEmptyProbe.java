import tlc2.TLCGlobals;
import tlc2.tool.TLCState;
import tlc2.tool.queue.DiskStateQueue;
import tlc2.tool.queue.StateQueue;
import java.nio.file.*;
import java.lang.reflect.*;
import java.util.*;
import java.util.concurrent.*;
public final class QueueEmptyProbe {
 public static void main(String[] args) throws Exception {
  Field stop=StateQueue.class.getDeclaredField("stop");stop.setAccessible(true);
  Path root=Paths.get(args[0]);Files.createDirectories(root);
  for(int width:new int[]{1,4,8}) for(int repeat=0;repeat<8;repeat++) {
   TLCGlobals.setNumWorkers(width);
   DiskStateQueue q=new DiskStateQueue(Files.createDirectory(root.resolve(width+"-"+repeat)).toString());
   ExecutorService pool=Executors.newFixedThreadPool(width+1);
   CountDownLatch ready=new CountDownLatch(width),release=new CountDownLatch(1);
   List<Future<Boolean>> workers=new ArrayList<>();
   try {
    for(int i=0;i<width;i++) workers.add(pool.submit(()->{ready.countDown();release.await();
     TLCState state=q.sDequeue();if(state!=null)throw new AssertionError("empty returned a state");q.finishAll();return true;}));
    if(!ready.await(5,TimeUnit.SECONDS))throw new TimeoutException("workers not ready");
    Future<Boolean> controller=pool.submit(()->q.suspendAll());
    long end=System.nanoTime()+TimeUnit.SECONDS.toNanos(5);
    for(;;){synchronized(q){if(stop.getBoolean(q))break;}if(System.nanoTime()>end)throw new TimeoutException("stop not observed");Thread.yield();}
    release.countDown();boolean suspended=controller.get(5,TimeUnit.SECONDS);
    int finished=0;for(Future<Boolean> worker:workers)if(worker.get(5,TimeUnit.SECONDS))finished++;
    if(suspended||finished!=width||q.size()!=0)throw new AssertionError("empty completion");
    System.out.printf("{\"width\":%d,\"repeat\":%d,\"suspended\":false,\"finished\":%d,\"size\":0}%n",width,repeat,finished);
   } finally {q.finishAll();release.countDown();pool.shutdown();if(!pool.awaitTermination(5,TimeUnit.SECONDS))throw new TimeoutException("cleanup");}
  }
 }
}

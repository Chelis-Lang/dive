import java.util.concurrent.*;
import java.util.concurrent.atomic.*;
import tlc2.value.impl.*;
/** Deliberate controlled instrumentation, not a claimed pure spec operator. */
public final class Progress {
 static final CountDownLatch expanding=new CountDownLatch(1),release=new CountDownLatch(1);
 static final AtomicBoolean first=new AtomicBoolean(true);
 static String scenario;
 public static Value Gate(Value v)throws Exception {
  ProgressProbe.event("actual-next-gate:"+Thread.currentThread().getName());
  if(!(scenario.equals("natural")||scenario.equals("natural-safe")||scenario.equals("final-eof"))&&first.compareAndSet(true,false)){
   expanding.countDown();if(!release.await(5,TimeUnit.SECONDS))throw new AssertionError("release timeout");
  }
  ProgressProbe.event("actual-next-gate-released:"+Thread.currentThread().getName());
  return BoolValue.ValTrue;
 }
}

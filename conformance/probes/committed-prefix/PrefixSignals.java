package tlc2.tool.impl;
import java.util.concurrent.*;
import java.util.concurrent.atomic.*;
import tlc2.tool.*;
import tlc2.value.impl.*;
import util.UniqueString;
public final class PrefixSignals {
 public static final CountDownLatch early=new CountDownLatch(1),later=new CountDownLatch(1),release=new CountDownLatch(1);
 public static final AtomicBoolean first=new AtomicBoolean(true);
 public static String scenario;
 public static int width;
 public static synchronized void event(String s){System.out.println("EVENT\t"+s);}
 public static int x(TLCState s){return ((IntValue)s.lookup(UniqueString.uniqueStringOf("x"))).val;}
 public static void gate(int x)throws Exception {
  event("actual-next-gate:"+x+":"+Thread.currentThread().getName());
  if(!scenario.equals("actual-eof")&&x==0&&first.compareAndSet(true,false)){
   early.countDown();if(!release.await(8,TimeUnit.SECONDS))throw new AssertionError("early release timeout");
  }
  if(x==width+1)later.countDown();
  event("actual-next-gate-released:"+x+":"+Thread.currentThread().getName());
 }
 public static void returned(TLCState s){
  if(scenario.equals("output-before-write")&&x(s)==1){
   later.countDown();try{if(!release.await(8,TimeUnit.SECONDS))throw new AssertionError("output release timeout");}
   catch(InterruptedException e){throw new RuntimeException(e);}
  }
 }
}

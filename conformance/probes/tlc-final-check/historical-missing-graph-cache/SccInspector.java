package tlc2.tool.liveness;

import java.io.*;
import java.lang.reflect.*;
import java.util.*;
import java.util.concurrent.*;
import tlc2.tool.*;
import tlc2.util.*;
import tlc2.util.statistics.*;

/** Invoke actual pinned private SCC/component methods with observing graphs.
 * FinalProbe also invokes the unmodified original LiveCheck.finalCheck.
 * The observing copy retains final adjacency/roots; it does not claim identical
 * original disk offsets or write history. All private-method rows are projections.
 */
public final class SccInspector {
 static String key(long fp,int t){return Long.toUnsignedString(fp)+":"+t;}
 static String link(long value){return value<0?"undone":value<AbstractDiskGraph.MAX_PTR?"file":value==AbstractDiskGraph.MAX_LINK?"done":"link"+(value-AbstractDiskGraph.MAX_PTR);}
 static final class Log {
  boolean enabled=true;
  final java.util.List<String> events=new ArrayList<>();
  void add(String value){if(enabled)events.add(value);}
 }
 static final class Plain extends DiskGraph {
  final Log log;
  Plain(String dir,int id,Log log)throws IOException{super(dir,id,new DummyBucketStatistics());this.log=log;}
  public long getLink(long fp,int t){long v=super.getLink(fp,t);log.add("get:"+key(fp,t)+":"+link(v));return v;}
  public long putLink(long fp,int t,long value){long v=super.putLink(fp,t,value);log.add("put:"+key(fp,t)+":"+link(v)+":"+link(value));return v;}
  public void setMaxLink(long fp,int t){super.setMaxLink(fp,t);log.add("max:"+key(fp,t));}
 }
 static final class Product extends TableauDiskGraph {
  final Log log;
  Product(String dir,int id,Log log)throws IOException{super(dir,id,new DummyBucketStatistics());this.log=log;}
  public long getLink(long fp,int t){long v=super.getLink(fp,t);log.add("get:"+key(fp,t)+":"+link(v));return v;}
  public long putLink(long fp,int t,long value){long v=super.putLink(fp,t,value);log.add("put:"+key(fp,t)+":"+link(v)+":"+link(value));return v;}
  public void setMaxLink(long fp,int t){super.setMaxLink(fp,t);log.add("max:"+key(fp,t));}
 }
 static void set(Object object,String name,Object value)throws Exception{Field f=LiveWorker.class.getDeclaredField(name);f.setAccessible(true);f.set(object,value);}
 static void err(int value)throws Exception{Field f=LiveWorker.class.getDeclaredField("errFoundByThread");f.setAccessible(true);f.setInt(null,value);}
 static Object invoke(LiveWorker worker,String method,Class<?>[] types,Object...args)throws Exception{
  Method m=LiveWorker.class.getDeclaredMethod(method,types);m.setAccessible(true);
  try{return m.invoke(worker,args);}catch(InvocationTargetException e){Throwable cause=e.getCause();if(cause instanceof Exception)throw (Exception)cause;throw e;}
 }
 static java.util.List<java.util.List<String>> components(java.util.List<String> events){
  java.util.List<java.util.List<String>> out=new ArrayList<>();java.util.List<String> pending=null;
  for(String event:events){if(event.startsWith("max:")){if(pending==null){pending=new ArrayList<>();out.add(pending);}pending.add(event.substring(4));}else pending=null;}
  return out;
 }
 public static void inspect(ITool tool,LiveCheck live,OrderOfSolution[] groups,java.util.List<TLCState> states,String dir)throws Exception{
  for(int g=0;g<groups.length;g++){
   OrderOfSolution o=groups[g];AbstractDiskGraph original=live.getChecker(g).getDiskGraph();
   for(int p=0;p<o.getPems().length;p++){
    String prefix="scc.g"+g+".pem"+p;File folder=new File(dir,"inspect-"+g+"-"+p);folder.mkdirs();Log log=new Log();log.enabled=false;
    AbstractDiskGraph disk=o.hasTableau()?new Product(folder.toString(),0,log):new Plain(folder.toString(),0,log);
    Map<String,Long> ptrs=new HashMap<>();
    LongVec roots=original.getInitNodes();for(int i=0;i<roots.size();i+=2)disk.addInitNode(roots.elementAt(i),(int)roots.elementAt(i+1));
    for(TLCState state:states){long fp=state.fingerPrint();int count=o.hasTableau()?o.getTableau().size():1;
     for(int t=0;t<count;t++){int tidx=o.hasTableau()?t:-1;long old=original.getPtr(fp,tidx);if(old<0)continue;
      GraphNode node=original.getNode(fp,tidx,old);long ptr=disk.addNode(node);ptrs.put(key(fp,tidx),ptr);
     }
    }
    LiveWorker worker=new LiveWorker(tool,0,1,live,new LinkedBlockingQueue<ILiveChecker>(),true);set(worker,"oos",o);set(worker,"pem",o.getPems()[p]);set(worker,"dg",disk);
    err(1); // Keep actual SCC traversal/return, suppress only trace printing.
    log.enabled=true;invoke(worker,"checkSccs",new Class<?>[]{ITool.class},tool);log.enabled=false;
    FinalProbe.field(prefix+".events",String.join(";",log.events));
    java.util.List<java.util.List<String>> cc=components(log.events);FinalProbe.field(prefix+".components",cc.size());
    for(int c=0;c<cc.size();c++){
     java.util.List<String> members=cc.get(c);String anchor=members.get(members.size()-1);String[] root=anchor.split(":");long fp=Long.parseUnsignedLong(root[0]);int tidx=Integer.parseInt(root[1]);
     MemIntStack stack=new MemIntStack(folder.toString(),"component-"+c);
     for(int i=members.size()-1;i>=0;i--){String[] item=members.get(i).split(":");stack.pushLong(ptrs.get(members.get(i)));stack.pushInt(Integer.parseInt(item[1]));stack.pushLong(Long.parseUnsignedLong(item[0]));}
     boolean ok=(Boolean)invoke(worker,"checkComponent",new Class<?>[]{ITool.class,long.class,int.class,IntStack.class},tool,fp,tidx,stack);
     FinalProbe.field(prefix+".component."+c+".anchor",anchor);FinalProbe.field(prefix+".component."+c+".pop_order",String.join(";",members));FinalProbe.field(prefix+".component."+c+".accepting",!ok);
    }
    disk.close();err(-1);
   }
  }
 }
}

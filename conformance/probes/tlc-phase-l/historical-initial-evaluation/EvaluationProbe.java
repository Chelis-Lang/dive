package tlc2.tool.liveness;
import java.util.*;
import java.nio.file.*;
import tlc2.tool.*;
import tlc2.util.*;
import tlc2.util.statistics.*;
import tlc2.value.impl.IntValue;

/** Invokes actual LiveCheck methods. Synthetic bins are explicitly diagnostics,
 * with no TLA-model, trace-validation or SCC-acceptance claim. */
public final class EvaluationProbe {
 static List<String> log=new ArrayList<>();static int fail;
 static boolean eval(String label,TLCState before,TLCState after){
  String target=after==null?"-":after.toString();
  String event=label+":"+before+":"+target;log.add(event);
  if(log.size()==fail)throw new IllegalStateException("diagnostic failure at "+event);
  return ((IntValue)((SlotsProbe.State)(after==null?before:after)).value).val%2==0;
 }
 static final class S extends LNState {
  final String label;S(String l){super(Context.Empty);label=l;}
  public boolean eval(ITool t,TLCState a,TLCState b){return EvaluationProbe.eval(label,a,null);}
 }
 static final class A extends LNAction {
  final String label;A(String l){super(null,Context.Empty);label=l;}
  public boolean eval(ITool t,TLCState a,TLCState b){return EvaluationProbe.eval(label,a,b);}
 }
 public static void main(String[] args)throws Exception {
  int groups=Integer.parseInt(args[0]),states=Integer.parseInt(args[1]),actions=Integer.parseInt(args[2]);fail=Integer.parseInt(args[3]);
  SetOfStates next=new SetOfStates(Integer.parseInt(args[4]));
  if(!args[5].isEmpty())for(String record:args[5].split(";")){
   String[] f=record.split(":",3);next.put(new SlotsProbe.State(Long.parseUnsignedLong(f[0]),f[1],SlotsProbe.value(f[2])));
  }
  SlotsProbe.State source=new SlotsProbe.State(0,"source",IntValue.gen(0));next.put(source);
  OrderOfSolution[] orders=new OrderOfSolution[groups];
  for(int g=0;g<groups;g++){
   OrderOfSolution o=new OrderOfSolution(new LNEven[0]);LiveExprNode[] ss=new LiveExprNode[states],aa=new LiveExprNode[actions];
   for(int i=0;i<states;i++)ss[i]=new S("g"+g+".s"+i);for(int i=0;i<actions;i++)aa[i]=new A("g"+g+".a"+i);
   o.setCheckState(ss);o.setCheckAction(aa);o.setPems(new PossibleErrorModel[]{new PossibleErrorModel(new int[0],new int[0],new int[0])});orders[g]=o;
  }
  LiveCheck check=new LiveCheck(null,orders,args[6],new DummyBucketStatistics());
  try{
   check.addInitState(null,source,0);System.out.println("initial.log\t"+String.join(";",log));
   check.addNextState(null,source,0,next);System.out.println("completion\tbin-and-disk-insertion");
  }catch(RuntimeException e){System.out.println("failure_class\t"+e.getClass().getName());System.out.println("failure_message\t"+e.getMessage());}
  finally{check.close();}
  System.out.println("log\t"+String.join(";",log));
  System.out.println("slots\t"+SlotsProbe.contents(next));System.out.println("capacity\t"+next.capacity());System.out.println("count\t"+next.size());
 }
}

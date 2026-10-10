package tlc2.tool.liveness;
import java.lang.reflect.*;
import java.util.*;
import tlc2.tool.*;
import tlc2.tool.impl.*;
import tlc2.util.*;
import tlc2.util.statistics.*;
import tlc2.value.impl.*;
import tlc2.value.RandomEnumerableValues;
import util.*;

/** Actual model-bound FP, SetOfStates and LiveCheck invocation projections. */
public final class ModelProbe {
 static java.util.List<String> log=new ArrayList<>();
 static void field(String k,Object v){System.out.println(k+"\t"+v);}
 static final class Bin extends LiveExprNode {
  final LiveExprNode original;final String label;
  Bin(LiveExprNode n,String l){original=n;label=l;}
  public int getLevel(){return original.getLevel();}
  public boolean containAction(){return original.containAction();}
  public boolean equals(LiveExprNode n){return original.equals(n);}
  public void toString(StringBuffer b,String p){original.toString(b,p);}
  public boolean eval(ITool t,TLCState a,TLCState b){
   int at=log.size();log.add(label+":pending");
   try{boolean value=original.eval(t,a,b);log.set(at,label+":"+value);return value;}
   catch(RuntimeException e){log.set(at,label+":error");throw e;}
  }
 }
 static TLCState state(ITool tool,String text){
  TLCState s=TLCState.Empty.createEmpty();String[] vals=text.split(",");
  tla2sany.semantic.OpDeclNode[] vars=s.getVars();
  if(vals.length!=vars.length)throw new IllegalArgumentException("state width");
  for(int i=0;i<vars.length;i++)s.bind(vars[i].getName(),SlotsProbe.value(vals[i]));
  return s;
 }
 public static void main(String[] args)throws Exception {
  try{run(args);}catch(tlc2.tool.EvalException e){field("failure_code",e.getErrorCode());field("failure_phase","planning");throw e;}
 }
 static void run(String[] args)throws Exception {
  java.io.PrintStream out=System.out;System.setOut(System.err);ToolIO.out=System.err;
  ToolIO.setUserDir(System.getProperty("user.dir"));FP64.Init(0);RandomEnumerableValues.setSeed(0);
  Tool tool=new FastTool(args[0],args[0],new SimpleFilenameToStream(),Tool.Mode.MC,new HashMap<>());
  System.setOut(out);
  OrderOfSolution[] groups=Liveness.processLiveness(tool,true);
  java.util.List<String> actionNames=new ArrayList<>();for(tlc2.tool.Action a:tool.getImpliedTemporals())actionNames.add(a.getName().toString());
  field("action_names",String.join(";",actionNames));field("role_names",String.join(";",tool.getImpliedTemporalNames()));
  java.util.List<String> configured=new ArrayList<>();tlc2.util.Vect<String> ns=tool.getModelConfig().getProperties();for(int i=0;i<ns.size();i++)configured.add(ns.elementAt(i));field("configured_names",String.join(";",configured));
  for(int g=0;g<groups.length;g++) {
   OrderOfSolution o=groups[g];LiveExprNode[] ss=o.getCheckState(),aa=o.getCheckAction();
   for(int i=0;i<ss.length;i++)ss[i]=new Bin(ss[i],"g"+g+".s"+i);
   for(int i=0;i<aa.length;i++)aa[i]=new Bin(aa[i],"g"+g+".a"+i);
   if(o.hasTableau()){
    Field f=TBGraphNode.class.getDeclaredField("statePreds");f.setAccessible(true);
    TBGraph t=o.getTableau();for(int n=0;n<t.size();n++){
     LiveExprNode[] predicates=(LiveExprNode[])f.get(t.getNode(n));
     for(int i=0;i<predicates.length;i++)predicates[i]=new Bin(predicates[i],"g"+g+".n"+n+".c"+i);
    }
   }
   field("g"+g+".tableau",o.hasTableau());field("g"+g+".initial",o.hasTableau()?o.getTableau().getInitCnt():0);
  }
  TLCState source=state(tool,args[1]);field("source.fp",Long.toUnsignedString(source.fingerPrint(tool)));
  SetOfStates next=new SetOfStates(Integer.parseInt(args[3]));
  int i=0;for(String text:args[2].split(";"))if(!text.isEmpty()){
   TLCState successor=state(tool,text);long fp=successor.fingerPrint(tool);field("incoming."+i+".fp",Long.toUnsignedString(fp));field("incoming."+i+".duplicate",next.put(fp,successor,tool));i++;
  }
  field("stutter.duplicate",next.put(source.fingerPrint(tool),source,tool));field("capacity",next.capacity());field("count",next.size());
  Field slotsField=SetOfStates.class.getDeclaredField("states");slotsField.setAccessible(true);
  TLCState[] slots=(TLCState[])slotsField.get(next);java.util.List<String> contents=new ArrayList<>();
  for(int s=0;s<slots.length;s++)if(slots[s]!=null)contents.add(s+":"+Long.toUnsignedString(slots[s].fingerPrint(tool)));
  field("slots",String.join(";",contents));
  LiveCheck check=new LiveCheck(tool,groups,args[4],new DummyBucketStatistics());
  try{
   check.addInitState(tool,source,source.fingerPrint(tool));field("initial.log",String.join(";",log));log.clear();
   check.addNextState(tool,source,source.fingerPrint(tool),next);field("completion","bin-and-disk-insertion");
  }catch(util.Assert.TLCRuntimeException e){field("failure_code",e.errorCode);field("failure_message",e.getMessage());field("failure_phase","bin-evaluation");}
  catch(tlc2.tool.EvalException e){field("failure_code",e.getErrorCode());field("failure_message",e.getMessage());field("failure_phase","bin-evaluation");}
  finally{check.close();field("log",String.join(";",log));}
 }
}

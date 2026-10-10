package tlc2.tool.liveness;
import java.lang.reflect.*;
import java.util.*;
import tlc2.tool.*;
import tlc2.tool.impl.*;
import tlc2.util.*;
import tlc2.util.statistics.*;
import tlc2.value.RandomEnumerableValues;
import util.*;

/** Bounded actual LiveCheck graph insertion; no ModelChecker or SCC verdict. */
public final class GraphProbe {
 static void field(String key,Object value){System.out.println(key+"\t"+value);}
 static String unsigned(long fp){return Long.toUnsignedString(fp);}
 static String bits(GraphNode n,int stateLen){StringBuilder s=new StringBuilder();for(int j=0;j<stateLen;j++)s.append(n.getCheckState(j)?'1':'0');return s.toString();}
 static String transitions(GraphNode n,int slen,int alen){List<String> out=new ArrayList<>();for(int i=0;i<n.succSize();i++){StringBuilder bits=new StringBuilder();for(int j=0;j<alen;j++)bits.append(n.getCheckAction(slen,alen,i,j)?'1':'0');out.add(unsigned(n.getStateFP(i))+":"+n.getTidx(i)+":"+bits);}return String.join(";",out);}
 public static void main(String[] args)throws Exception {
  java.io.PrintStream out=System.out;System.setOut(System.err);ToolIO.out=System.err;
  ToolIO.setUserDir(System.getProperty("user.dir"));FP64.Init(0);RandomEnumerableValues.setSeed(0);
  Tool tool=new FastTool(args[0],args[0],new SimpleFilenameToStream(),Tool.Mode.MC,new HashMap<>());
  System.setOut(out);OrderOfSolution[] groups=Liveness.processLiveness(tool,true);
  List<String> names=new ArrayList<>();for(Action a:tool.getImpliedTemporals())names.add(a.getName().toString());field("action_names",String.join(";",names));field("role_names",String.join(";",tool.getImpliedTemporalNames()));
  List<String> configured=new ArrayList<>();Vect<String> config=tool.getModelConfig().getProperties();for(int i=0;i<config.size();i++)configured.add(config.elementAt(i));field("configured_names",String.join(";",configured));field("groups",groups.length);
  if(groups.length!=1||groups[0].hasTableau()){field("completion","outside-null-one-group-projection");return;}
  OrderOfSolution order=groups[0];LiveExprNode[] ss=order.getCheckState(),aa=order.getCheckAction();
  for(int i=0;i<ss.length;i++)ss[i]=new ModelProbe.Bin(ss[i],"g0.s"+i);for(int i=0;i<aa.length;i++)aa[i]=new ModelProbe.Bin(aa[i],"g0.a"+i);
  field("state_bins",ss.length);field("action_bins",aa.length);
  LinkedHashMap<Long,TLCState> visited=new LinkedHashMap<>();List<TLCState> queue=new ArrayList<>();List<Long> roots=new ArrayList<>();
  LiveCheck live=new LiveCheck(tool,groups,args[1],new DummyBucketStatistics());DiskGraph disk=(DiskGraph)live.getChecker(0).getDiskGraph();long generated=0;int multiplier=1;
  try {
   StateVec inits=tool.getInitStates();generated+=inits.size();for(int i=0;i<inits.size();i++){TLCState s=inits.elementAt(i);if(tool.isInModel(s)){long fp=s.fingerPrint(tool);if(!visited.containsKey(fp)){visited.put(fp,s);queue.add(s);roots.add(fp);live.addInitState(tool,s,fp);}}}
   field("initial.log",String.join(";",ModelProbe.log));ModelProbe.log.clear();
   for(int index=0;index<queue.size();index++) {
    if(queue.size()>128||generated>8192)throw new IllegalStateException("probe-entry-bound");
    TLCState source=queue.get(index);long fp=source.fingerPrint(tool);SetOfStates next=new SetOfStates(16*multiplier);int ordinal=0;List<String> incoming=new ArrayList<>();
    for(Action action:tool.getActions()) {
     StateVec successors=tool.getNextStates(action,source);
     for(int j=0;j<successors.size();j++){TLCState target=successors.elementAt(j);generated++;long targetFP=target.fingerPrint(tool);if(tool.isInModel(target)&&tool.isInActions(source,target)){incoming.add(ordinal+":"+unsigned(targetFP));next.put(targetFP,target,tool);if(!visited.containsKey(targetFP)){visited.put(targetFP,target);queue.add(target);}}ordinal++;}
    }
    next.put(fp,source,tool);field("source."+index+".fp",unsigned(fp));field("source."+index+".incoming",String.join(";",incoming));field("source."+index+".capacity",next.capacity());field("source."+index+".count",next.size());
    Field f=SetOfStates.class.getDeclaredField("states");f.setAccessible(true);TLCState[] slots=(TLCState[])f.get(next);List<String> positions=new ArrayList<>();for(int i=0;i<slots.length;i++)if(slots[i]!=null)positions.add(i+":"+unsigned(slots[i].fingerPrint(tool)));field("source."+index+".slots",String.join(";",positions));
    List<String> before=new ArrayList<>();for(int i=0;i<slots.length;i++)if(slots[i]!=null){long dest=slots[i].fingerPrint(tool);before.add(unsigned(dest)+":"+(disk.getPtr(dest)!=-1));}field("source."+index+".target_written_before",String.join(";",before));
    live.addNextState(tool,source,fp,next);GraphNode node=disk.getNode(fp);field("source."+index+".states",bits(node,ss.length));field("source."+index+".transitions",transitions(node,ss.length,aa.length));field("source."+index+".log",String.join(";",ModelProbe.log));ModelProbe.log.clear();
    if(next.capacity()>16*multiplier)multiplier++;field("source."+index+".next_multiplier",multiplier);
   }
   List<String> rr=new ArrayList<>();LongVec actual=disk.getInitNodes();for(int i=0;i<actual.size();i+=2)rr.add(unsigned(actual.elementAt(i))+":"+actual.elementAt(i+1));field("roots",String.join(";",rr));field("generated",generated);field("distinct",queue.size());field("completion","graph-inserted-no-scc-proof");
  } catch(tlc2.tool.EvalException e){field("failure_code",e.getErrorCode());field("log",String.join(";",ModelProbe.log));throw e;}
  finally {live.close();}
 }
}

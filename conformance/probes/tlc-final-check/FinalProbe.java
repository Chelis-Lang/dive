package tlc2.tool.liveness;
import java.lang.reflect.*;
import java.util.*;
import tlc2.tool.*;
import tlc2.tool.impl.*;
import tlc2.util.*;
import tlc2.util.statistics.*;
import tlc2.value.RandomEnumerableValues;
import util.*;

/** Actual bounded multiple-OOS/tableau LiveCheck insertion, no SCC verdict. */
public final class FinalProbe {
 static void field(String key,Object value){System.out.println(key+"\t"+value);}
 static String unsigned(long fp){return Long.toUnsignedString(fp);}
 static String bits(GraphNode n,int len){StringBuilder s=new StringBuilder();for(int j=0;j<len;j++)s.append(n.getCheckState(j)?'1':'0');return s.toString();}
 static String transitions(GraphNode n,int slen,int alen){java.util.List<String> out=new ArrayList<>();for(int i=0;i<n.succSize();i++){StringBuilder b=new StringBuilder();for(int j=0;j<alen;j++)b.append(n.getCheckAction(slen,alen,i,j)?'1':'0');out.add(unsigned(n.getStateFP(i))+":"+n.getTidx(i)+":"+b);}return String.join(";",out);}
 static void graph(LiveCheck live,OrderOfSolution[] groups,java.util.List<TLCState> queue,String prefix)throws Exception {
  for(int g=0;g<groups.length;g++){
   OrderOfSolution o=groups[g];AbstractDiskGraph disk=live.getChecker(g).getDiskGraph();
   java.util.List<String> roots=new ArrayList<>();LongVec rr=disk.getInitNodes();for(int i=0;i<rr.size();i+=2)roots.add(unsigned(rr.elementAt(i))+":"+rr.elementAt(i+1));field(prefix+".g"+g+".roots",String.join(";",roots));
   for(int s=0;s<queue.size();s++){
    long fp=queue.get(s).fingerPrint();int count=o.hasTableau()?o.getTableau().size():1;
    java.util.List<String> registered=new ArrayList<>();
    for(int t=0;t<count;t++){
     int tidx=o.hasTableau()?t:-1;
     long ptr=o.hasTableau()?((TableauDiskGraph)disk).getPtr(fp,tidx):((DiskGraph)disk).getPtr(fp);
     if(ptr==-1)continue;registered.add(tidx+":"+(ptr>=0));
     if(ptr>=0){GraphNode n=o.hasTableau()?((TableauDiskGraph)disk).getNode(fp,tidx):((DiskGraph)disk).getNode(fp);String key=prefix+".g"+g+".node."+unsigned(fp)+"."+tidx;field(key+".states",bits(n,o.getCheckState().length));field(key+".transitions",transitions(n,o.getCheckState().length,o.getCheckAction().length));}
    }
    field(prefix+".g"+g+".records."+unsigned(fp),String.join(";",registered));
    if(o.hasTableau()){
     Field f=TableauDiskGraph.class.getDeclaredField("nodePtrTbl");f.setAccessible(true);TableauNodePtrTable table=(TableauNodePtrTable)f.get(disk);int[] record=table.getNodes(fp);java.util.List<String> order=new ArrayList<>();if(record!=null)for(int i=2;i<record.length;i+=table.getElemLength())order.add(Integer.toString(record[i]));field(prefix+".g"+g+".record_order."+unsigned(fp),String.join(";",order));
    }
   }
  }
 }
 public static void main(String[] args)throws Exception {
  java.io.PrintStream out=System.out;System.setOut(System.err);ToolIO.out=System.err;
  ToolIO.setUserDir(System.getProperty("user.dir"));FP64.Init(0);RandomEnumerableValues.setSeed(0);tlc2.TLCGlobals.setNumWorkers(1);tlc2.TLCGlobals.tool=true;
  Tool tool=new FastTool(args[0],args[0],new SimpleFilenameToStream(),Tool.Mode.MC,new HashMap<>());
  System.setOut(out);OrderOfSolution[] groups=Liveness.processLiveness(tool,true);field("groups",groups.length);
  java.util.List<String> names=new ArrayList<>();for(Action a:tool.getImpliedTemporals())names.add(a.getName().toString());field("action_names",String.join(";",names));field("role_names",String.join(";",tool.getImpliedTemporalNames()));
  java.util.List<String> configured=new ArrayList<>();Vect<String> config=tool.getModelConfig().getProperties();for(int i=0;i<config.size();i++)configured.add(config.elementAt(i));field("configured_names",String.join(";",configured));
  for(int g=0;g<groups.length;g++){
   LiveExprNode[] ss=groups[g].getCheckState(),aa=groups[g].getCheckAction();
   for(int i=0;i<ss.length;i++)ss[i]=new ModelProbe.Bin(ss[i],"g"+g+".s"+i);for(int i=0;i<aa.length;i++)aa[i]=new ModelProbe.Bin(aa[i],"g"+g+".a"+i);
   field("g"+g+".state_bins",ss.length);field("g"+g+".action_bins",aa.length);field("g"+g+".tableau",groups[g].hasTableau());
  }
  LinkedHashMap<Long,TLCState> visited=new LinkedHashMap<>();java.util.List<TLCState> queue=new ArrayList<>();
  LiveCheck live=new LiveCheck(tool,groups,args[1],new DummyBucketStatistics());long generated=0;int multiplier=1;
  try {
   StateVec inits=tool.getInitStates();generated+=inits.size();for(int i=0;i<inits.size();i++){TLCState s=inits.elementAt(i);if(tool.isInModel(s)){long fp=s.fingerPrint(tool);if(!visited.containsKey(fp)){visited.put(fp,s);queue.add(s);live.addInitState(tool,s,fp);}}}
   field("initial.log",String.join(";",ModelProbe.log));ModelProbe.log.clear();graph(live,groups,queue,"initial");
   for(int index=0;index<queue.size();index++){
    if(queue.size()>128||generated>8192)throw new IllegalStateException("probe-entry-bound");
    TLCState source=queue.get(index);long fp=source.fingerPrint(tool);SetOfStates next=new SetOfStates(16*multiplier);int ordinal=0;java.util.List<String> incoming=new ArrayList<>();
    for(Action action:tool.getActions()){
     StateVec successors=tool.getNextStates(action,source);
     for(int j=0;j<successors.size();j++){TLCState target=successors.elementAt(j);generated++;if(tool.isInModel(target)&&tool.isInActions(source,target)){long targetFP=target.fingerPrint(tool);incoming.add(ordinal+":"+unsigned(targetFP));next.put(targetFP,target,tool);if(!visited.containsKey(targetFP)){visited.put(targetFP,target);queue.add(target);}}ordinal++;}
    }
    next.put(fp,source,tool);field("source."+index+".fp",unsigned(fp));field("source."+index+".incoming",String.join(";",incoming));field("source."+index+".capacity",next.capacity());
    live.addNextState(tool,source,fp,next);field("source."+index+".log",String.join(";",ModelProbe.log));ModelProbe.log.clear();graph(live,groups,queue,"source."+index);
    if(next.capacity()>16*multiplier)multiplier++;field("source."+index+".next_multiplier",multiplier);
   }
   graph(live,groups,queue,"final");field("generated",generated);field("distinct",queue.size());field("completion","graph-inserted-no-scc-proof");SccInspector.inspect(tool,live,groups,queue,args[1]);field("final_check_code",live.finalCheck(tool));
  }catch(tlc2.tool.EvalException e){field("failure_code",e.getErrorCode());field("log",String.join(";",ModelProbe.log));throw e;}
  finally {live.close();}
 }
}

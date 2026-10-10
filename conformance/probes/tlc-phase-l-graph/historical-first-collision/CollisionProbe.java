package tlc2.tool.liveness;
import java.util.HashMap;
import tlc2.tool.*;
import tlc2.tool.impl.*;
import tlc2.util.*;
import tlc2.util.statistics.*;
import tlc2.value.RandomEnumerableValues;
import util.*;

/** Forced fingerprint keys isolate ptr/transExists; no fingerprint parity claim. */
public final class CollisionProbe {
 public static void main(String[] args)throws Exception {
  java.io.PrintStream out=System.out;System.setOut(System.err);ToolIO.out=System.err;
  ToolIO.setUserDir(System.getProperty("user.dir"));FP64.Init(0);RandomEnumerableValues.setSeed(0);
  Tool tool=new FastTool("PhaseActionValid","PhaseActionValid",new SimpleFilenameToStream(),Tool.Mode.MC,new HashMap<>());
  System.setOut(out);OrderOfSolution[] groups=Liveness.processLiveness(tool,true);
  LiveCheck live=new LiveCheck(tool,groups,args[0],new DummyBucketStatistics());DiskGraph disk=(DiskGraph)live.getChecker(0).getDiskGraph();
  TLCState zero=ModelProbe.state(tool,"0"),one=ModelProbe.state(tool,"1"),two=ModelProbe.state(tool,"2");
  try{
   live.addInitState(tool,zero,100L);
   for(int step=0;step<3;step++){
    TLCState source=step==1?one:zero;long fp=step==1?42L:100L;SetOfStates next=new SetOfStates(16);
    next.put(42L,one,tool);next.put(42L,two,tool);next.put(fp,source,tool);
    live.addNextState(tool,source,fp,next);GraphNode node=disk.getNode(fp);
    GraphProbe.field("step."+step+".fp",fp);GraphProbe.field("step."+step+".transitions",GraphProbe.transitions(node,groups[0].getCheckState().length,groups[0].getCheckAction().length));
   }
   GraphProbe.field("completion","forced-collision-written-unwritten-projection-no-fingerprint-proof");
  }finally{live.close();}
 }
}

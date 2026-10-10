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
  OrderOfSolution order=new OrderOfSolution(new LNEven[0]);
  order.setCheckState(new LiveExprNode[]{new EvaluationProbe.Bin("s0",false)});
  order.setCheckAction(new LiveExprNode[]{new EvaluationProbe.Bin("a0",true)});
  order.setPems(new PossibleErrorModel[]{new PossibleErrorModel(new int[0],new int[0],new int[0])});
  OrderOfSolution[] groups=new OrderOfSolution[]{order};
  LiveCheck live=new LiveCheck(null,groups,args[0],new DummyBucketStatistics());DiskGraph disk=(DiskGraph)live.getChecker(0).getDiskGraph();
  TLCState zero=new SlotsProbe.State(100L,"zero",SlotsProbe.value("i0")),one=new SlotsProbe.State(42L,"one",SlotsProbe.value("i1")),two=new SlotsProbe.State(42L,"two",SlotsProbe.value("i2"));
  try{
   live.addInitState(null,zero,100L);
   for(int step=0;step<3;step++){
    TLCState source=step==1?one:zero;long fp=step==1?42L:100L;SetOfStates next=new SetOfStates(16);
    next.put(42L,one,null);next.put(42L,two,null);next.put(fp,source,null);
    live.addNextState(null,source,fp,next);GraphNode node=disk.getNode(fp);
    GraphProbe.field("step."+step+".fp",fp);GraphProbe.field("step."+step+".transitions",GraphProbe.transitions(node,groups[0].getCheckState().length,groups[0].getCheckAction().length));
   }
   GraphProbe.field("completion","forced-collision-written-unwritten-projection-no-fingerprint-proof");
  }finally{live.close();}
 }
}

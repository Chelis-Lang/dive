package tlc2.tool.liveness;
import tlc2.util.statistics.DummyBucketStatistics;
/** Actual pinned pointer-table API counts/snapshots; no model verdict. */
public final class TableProbe {
 static void emit(String key,AbstractDiskGraph d)throws Exception {
  System.out.println(key+".size\t"+d.size());
  System.out.println(key+".last\t"+d.getSizeAtLastCheck());
 }
 public static void main(String[] args)throws Exception {
  DiskGraph n=new DiskGraph(args[0],0,new DummyBucketStatistics());
  TableauDiskGraph t=new TableauDiskGraph(args[0],1,new DummyBucketStatistics());
  try {
   emit("null.empty",n);n.addInitNode(11,-1);emit("null.root",n);
   n.addNode(new GraphNode(11,-1));emit("null.write",n);
   n.addNode(new GraphNode(11,-1));emit("null.rewrite",n);
   n.recordSize();emit("null.snapshot",n);n.addNode(new GraphNode(22,-1));emit("null.growth",n);
   n.makeNodePtrTbl();emit("null.restore",n);
   emit("tableau.empty",t);t.addInitNode(11,0);emit("tableau.root",t);
   t.recordNode(11,0);emit("tableau.register",t);t.recordNode(11,1);emit("tableau.second_particle",t);
   t.setDone(22);emit("tableau.done_only",t);t.recordNode(33,7);emit("tableau.undone",t);
   t.addNode(new GraphNode(11,0));emit("tableau.write",t);t.recordSize();emit("tableau.snapshot",t);
   t.makeNodePtrTbl();emit("tableau.restore",t);
   System.out.println("tableau.restored_undone\t"+t.getPtr(33,7));
   System.out.println("tableau.restored_done_only\t"+t.isDone(22));
   n.reset();n.recordSize();emit("null.zero_snapshot",n);
   for(double threshold:new double[]{0.1,0.0,-0.1,Double.NaN,Double.POSITIVE_INFINITY,Double.NEGATIVE_INFINITY}){
    for(long[] pair:new long[][]{{0,0},{0,1},{1,0},{11,10},{12,10}}){
     double delta=(pair[0]-pair[1])/(pair[1]*1.d);
     System.out.println("due."+pair[0]+"."+pair[1]+"."+Double.toString(threshold)+"\t"+(delta>threshold));
    }
   }
  }finally{n.close();t.close();}
 }
}

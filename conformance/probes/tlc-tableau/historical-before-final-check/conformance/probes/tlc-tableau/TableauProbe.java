package tlc2.tool.liveness;

import java.lang.reflect.*;
import java.util.*;
import tlc2.tool.ITool;
import tlc2.tool.impl.FastTool;
import tlc2.tool.impl.Tool;
import tlc2.util.FP64;
import tlc2.value.RandomEnumerableValues;
import util.SimpleFilenameToStream;
import util.ToolIO;

/** Actual bounded-selection particle/tableau projections; no predicate eval. */
public final class TableauProbe {
  static void field(String key,Object value) {System.out.println(key+"\t"+value);}
  static String text(LiveExprNode n) {
    if(n instanceof LNNext)return "X("+text(((LNNext)n).getBody())+")";
    if(n instanceof LNConj||n instanceof LNDisj) {
      List<String> out=new ArrayList<>();boolean and=n instanceof LNConj;
      int count=and?((LNConj)n).getCount():((LNDisj)n).getCount();
      for(int i=0;i<count;i++)out.add(text(and?((LNConj)n).getBody(i):((LNDisj)n).getBody(i)));
      return (and?"C":"D")+"("+String.join(",",out)+")";
    }
    if(n instanceof LNAll)return "G("+text(((LNAll)n).getBody())+")";
    if(n instanceof LNEven)return "E("+text(((LNEven)n).getBody())+")";
    if(n instanceof LNNeg)return "N("+text(((LNNeg)n).getBody())+")";
    return PlanningProbe.text(n);
  }
  static String texts(LiveExprNode[] ns) {List<String> out=new ArrayList<>();for(LiveExprNode n:ns)out.add(text(n));return String.join(";",out);}
  static void graph(String prefix,TBGraph graph,LNEven[] promises)throws Exception {
    field(prefix+".binary",graph==null?"null":text(graph.tf));
    field(prefix+".promises",texts(promises));
    if(graph==null){field(prefix+".initial",0);field(prefix+".nodes",0);return;}
    field(prefix+".initial",graph.getInitCnt());field(prefix+".nodes",graph.size());
    Field states=TBGraphNode.class.getDeclaredField("statePreds");states.setAccessible(true);
    for(int i=0;i<graph.size();i++) {
      TBGraphNode node=graph.getNode(i);String key=prefix+".n"+i;
      List<String> particles=new ArrayList<>(),next=new ArrayList<>(),fulfilled=new ArrayList<>();
      for(int j=0;j<node.getPar().size();j++)particles.add(text(node.getPar().exprAt(j)));
      for(int j=0;j<node.nextSize();j++)next.add(""+node.nextAt(j).getIndex());
      for(LNEven promise:promises)fulfilled.add(""+node.getPar().isFulfilling(promise));
      field(key+".index",node.getIndex());field(key+".particle",String.join(";",particles));
      field(key+".state",texts((LiveExprNode[])states.get(node)));
      field(key+".next",String.join(",",next));field(key+".fulfills",String.join(",",fulfilled));field(key+".accepting",node.isAccepting());
    }
  }
  static void formulaGraph(String key,LiveExprNode root)throws Exception {
    LiveExprNode binary=root.makeBinary();graph(key,new TBGraph(binary),binary.extractPromises());
  }
  static final class DirectParser {
    final String source;int at;
    DirectParser(String source){this.source=source;}
    LiveExprNode parse(){
      char c=source.charAt(at++);
      if(c=='T')return LNBool.TRUE;if(c=='F')return LNBool.FALSE;
      if(c=='s'||c=='a'){
        int start=at;while(at<source.length()&&Character.isDigit(source.charAt(at)))at++;
        LiveExprNode atom=c=='s'?new PlanningProbe.State():new PlanningProbe.Action();
        atom.tagExpr(1+Integer.parseInt(source.substring(start,at)));return atom;
      }
      if(source.charAt(at++)!='(')throw new IllegalArgumentException(source);
      List<LiveExprNode> cs=new ArrayList<>();
      if(source.charAt(at)!=')'){do{cs.add(parse());}while(at<source.length()&&source.charAt(at++)==',');at--;}
      if(source.charAt(at++)!=')')throw new IllegalArgumentException(source);
      if(c=='N')return new LNNeg(cs.get(0));if(c=='G')return new LNAll(cs.get(0));if(c=='E')return new LNEven(cs.get(0));
      tlc2.util.Vect<LiveExprNode> out=new tlc2.util.Vect<>();for(LiveExprNode n:cs)out.addElement(n);
      if(c=='C')return new LNConj(out);if(c=='D')return new LNDisj(out);throw new IllegalArgumentException(source);
    }
  }
  static void supplied(String fair,String props)throws Exception {
    LNConj combined=new LNConj(0);if(!fair.isEmpty())for(String s:fair.split(";"))combined.addConj(new PlanningProbe.Parser(s).parse());
    LiveExprNode root;
    if(!props.isEmpty()){
      String[] ps=props.split(";");LiveExprNode checks;
      if(ps.length==1)checks=new LNNeg(new PlanningProbe.Parser(ps[0]).parse());
      else{LNDisj disj=new LNDisj(0);for(String s:ps)disj.addDisj(new LNNeg(new PlanningProbe.Parser(s).parse()));checks=disj;}
      if(fair.isEmpty())root=checks;else{combined.addConj(checks);root=combined;}
    }else{root=fair.isEmpty()?null:combined;}
    if(root==null){field("groups",0);field("empty",true);return;}
    root.tagExpr(1);root=root.pushNeg(false).simplify().toDNF();
    if(root instanceof LNBool&&!((LNBool)root).b){field("groups",0);return;}
    LNDisj disj=root instanceof LNDisj?(LNDisj)root:new LNDisj(root);
    Class<?> pem=Class.forName("tlc2.tool.liveness.Liveness$OSExprPem");Constructor<?> ctor=pem.getDeclaredConstructor();ctor.setAccessible(true);
    Method classify=Liveness.class.getDeclaredMethod("classifyExpr",LiveExprNode.class,pem);classify.setAccessible(true);
    Method temporal=pem.getDeclaredMethod("toTFS");temporal.setAccessible(true);
    List<LiveExprNode> groups=new ArrayList<>();
    for(int i=0;i<disj.getCount();i++){
      Object p=ctor.newInstance();LiveExprNode branch=disj.getBody(i).flattenSingleJunctions();
      if(branch instanceof LNConj){LNConj conj=(LNConj)branch;for(int j=0;j<conj.getCount();j++)classify.invoke(null,conj.getBody(j),p);}else classify.invoke(null,branch,p);
      LiveExprNode key=(LiveExprNode)temporal.invoke(p);boolean found=false;
      for(LiveExprNode other:groups)if(key==null?other==null:other!=null&&key.equals(other)){found=true;break;}
      if(!found)groups.add(key);
    }
    field("groups",groups.size());for(int i=0;i<groups.size();i++){
      if(groups.get(i)==null)graph("g"+i,null,new LNEven[0]);else formulaGraph("g"+i,groups.get(i));
    }
  }
  static void run(String[] args)throws Exception {
    if(args[0].equals("--model")){
      java.io.PrintStream out=System.out;System.setOut(System.err);ToolIO.out=System.err;
      ToolIO.setUserDir(System.getProperty("user.dir"));FP64.Init(0);RandomEnumerableValues.setSeed(0);
      Tool tool=new FastTool(args[1],args[2],new SimpleFilenameToStream(),Tool.Mode.MC,new HashMap<>());System.setOut(out);
      List<String> actions=new ArrayList<>();for(tlc2.tool.Action a:tool.getImpliedTemporals())actions.add(a.getName().toString());
      field("action_names",String.join(";",actions));field("role_names",String.join(";",tool.getImpliedTemporalNames()));
      List<String> names=new ArrayList<>();tlc2.util.Vect<String> configured=tool.getModelConfig().getProperties();for(int i=0;i<configured.size();i++)names.add(configured.elementAt(i));field("configured_names",String.join(";",names));
      OrderOfSolution[] groups=Liveness.processLiveness(tool,true);field("groups",groups.length);
      for(int i=0;i<groups.length;i++)graph("g"+i,groups[i].hasTableau()?groups[i].getTableau():null,groups[i].getPromises());return;
    }
    if(args[0].equals("--direct")){field("groups",1);formulaGraph("g0",new DirectParser(args[1]).parse());return;}
    supplied(args[0],args[1]);
  }
  public static void main(String[] args)throws Exception {
    try{run(args);}catch(InvocationTargetException e){if(e.getCause() instanceof util.Assert.TLCRuntimeException)field("failure_code",((util.Assert.TLCRuntimeException)e.getCause()).errorCode);throw e;}
    catch(util.Assert.TLCRuntimeException e){field("failure_code",e.errorCode);throw e;}
  }
}

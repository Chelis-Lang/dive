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
import tlc2.tool.TLCState;
import tlc2.util.Context;
import tlc2.util.Vect;

/** Development-only projection; predicates are inert, never evaluated. */
public final class PlanningProbe {
  static final class State extends LNState {
    State() { super(Context.Empty); }
    public boolean eval(ITool t,TLCState a,TLCState b) { throw new AssertionError("predicate evaluated"); }
    public void toString(StringBuffer b,String p) { b.append("s").append(getTag()); }
  }
  static final class Action extends LNAction {
    Action() { super(null,Context.Empty); }
  }
  static final class Parser {
    final String s; int at;
    Parser(String s) {this.s=s;}
    LiveExprNode parse() {
      char c=s.charAt(at++);
      if(c=='T')return LNBool.TRUE;
      if(c=='F')return LNBool.FALSE;
      if(c=='s'||c=='a') {while(at<s.length()&&Character.isDigit(s.charAt(at)))at++;return c=='s'?new State():new Action();}
      if(s.charAt(at++)!='(')throw new IllegalArgumentException(s);
      List<LiveExprNode> cs=new ArrayList<>();
      if(s.charAt(at)!=')') {
        do {cs.add(parse());} while(at<s.length()&&s.charAt(at++)==',');
        at--; // the loop consumed the closing parenthesis
      }
      if(s.charAt(at++)!=')')throw new IllegalArgumentException(s);
      if(c=='N')return new LNNeg(cs.get(0));
      if(c=='G')return new LNAll(cs.get(0));
      if(c=='E')return new LNEven(cs.get(0));
      Vect<LiveExprNode> v=new Vect<>();for(LiveExprNode n:cs)v.addElement(n);
      if(c=='C')return new LNConj(v);
      if(c=='D')return new LNDisj(v);
      throw new IllegalArgumentException(s);
    }
  }
  static String text(LiveExprNode n) {
    if(n instanceof LNBool)return ((LNBool)n).b?"T":"F";
    if(n instanceof State)return n.toString();
    if(n instanceof LNState)return "s"+((LNState)n).getTag();
    if(n instanceof LNAction)return "a"+((LNAction)n).getTag();
    if(n instanceof LNNeg)return "N("+text(((LNNeg)n).getBody())+")";
    if(n instanceof LNAll)return "G("+text(((LNAll)n).getBody())+")";
    if(n instanceof LNEven)return "E("+text(((LNEven)n).getBody())+")";
    List<String> cs=new ArrayList<>();String op;
    if(n instanceof LNConj){op="C";for(int i=0;i<((LNConj)n).getCount();i++)cs.add(text(((LNConj)n).getBody(i)));}
    else if(n instanceof LNDisj){op="D";for(int i=0;i<((LNDisj)n).getCount();i++)cs.add(text(((LNDisj)n).getBody(i)));}
    else throw new IllegalArgumentException(n.getClass().getName());
    return op+"("+String.join(",",cs)+")";
  }
  static Vect<LiveExprNode> get(Object p,String name)throws Exception {
    Field f=p.getClass().getDeclaredField(name);f.setAccessible(true);return (Vect<LiveExprNode>)f.get(p);
  }
  static int bin(LiveExprNode n,List<LiveExprNode> bin){for(int i=0;i<bin.size();i++)if(n.equals(bin.get(i)))return i;bin.add(n);return bin.size()-1;}
  static String indexes(Vect<LiveExprNode> ns,List<LiveExprNode> bin){List<String> out=new ArrayList<>();for(int i=0;i<ns.size();i++)out.add(""+bin(ns.elementAt(i),bin));return String.join(",",out);}
  static String texts(List<LiveExprNode> ns){List<String> out=new ArrayList<>();for(LiveExprNode n:ns)out.add(text(n));return String.join(";",out);}
  static void actual(ITool tool)throws Exception {
    OrderOfSolution[] orders=Liveness.processLiveness(tool,true);
    System.out.println("actual.groups\t"+orders.length);
    for(int i=0;i<orders.length;i++) {
      OrderOfSolution g=orders[i];
      System.out.println("actual.g"+i+".state\t"+texts(Arrays.asList(g.getCheckState())));
      System.out.println("actual.g"+i+".action\t"+texts(Arrays.asList(g.getCheckAction())));
      PossibleErrorModel[] pems=g.getPems();
      for(int j=0;j<pems.length;j++) {
        PossibleErrorModel p=pems[j];
        String aa=Arrays.toString(p.AEAction).replaceAll("[\\[\\] ]",""),as=Arrays.toString(p.AEState).replaceAll("[\\[\\] ]",""),ea=Arrays.toString(p.EAAction).replaceAll("[\\[\\] ]","");
        System.out.println("actual.g"+i+".p"+j+"\tAEAction="+aa+"|AEState="+as+"|EAAction="+ea);
      }
      List<LiveExprNode> promises=new ArrayList<>();for(LNEven p:g.getPromises())promises.add(p);
      System.out.println("actual.g"+i+".promises\t"+texts(promises));
      System.out.println("actual.g"+i+".tableauNodes\t"+(g.hasTableau()?g.getTableau().size():0));
    }
  }
  public static void main(String[] args)throws Exception {
    try {run(args);} catch(util.Assert.TLCRuntimeException e) {
      System.out.println("failure_code\t"+e.errorCode);throw e;
    }
  }
  static void run(String[] args)throws Exception {
    if(args[0].equals("--model")) {
      java.io.PrintStream out=System.out;
      System.setOut(System.err);ToolIO.out=System.err;
      ToolIO.setUserDir(System.getProperty("user.dir"));FP64.Init(0);RandomEnumerableValues.setSeed(0);
      Tool tool=new FastTool(args[1],args[2],new SimpleFilenameToStream(),Tool.Mode.MC,new HashMap<>());
      System.setOut(out);
      List<String> identities=new ArrayList<>();for(tlc2.tool.Action a:tool.getImpliedTemporals())identities.add(a.getName().toString());
      System.out.println("properties\t"+String.join(";",identities));
      System.out.println("temporal_role_names\t"+String.join(";",tool.getImpliedTemporalNames()));
      Vect<String> configured=tool.getModelConfig().getProperties();List<String> originals=new ArrayList<>();for(int i=0;i<configured.size();i++)originals.add(configured.elementAt(i));
      System.out.println("configured_properties\t"+String.join(";",originals));
      Method parse=Liveness.class.getDeclaredMethod("parseLiveness",ITool.class);parse.setAccessible(true);
      LiveExprNode root=(LiveExprNode)parse.invoke(null,tool);
      show(root);
      actual(tool);return;
    }
    LNConj combined=new LNConj(0);
    if(!args[0].isEmpty())for(String s:args[0].split(";"))combined.addConj(new Parser(s).parse());
    LiveExprNode root;
    if(!args[1].isEmpty()){
      String[] ps=args[1].split(";");
      LiveExprNode checks;
      if(ps.length==1)checks=new LNNeg(new Parser(ps[0]).parse());
      else {LNDisj disj=new LNDisj(0);for(String s:ps)disj.addDisj(new LNNeg(new Parser(s).parse()));checks=disj;}
      if(args[0].isEmpty())root=checks;else {combined.addConj(checks);root=combined;}
    }else {root=args[0].isEmpty()?null:combined;}
    if(root==null){System.out.println("empty\ttrue");return;}
    show(root);
  }
  static void show(LiveExprNode root)throws Exception {
    if(root==null){System.out.println("empty\ttrue");return;}
    root.tagExpr(1);System.out.println("tagged\t"+text(root));
    root=root.pushNeg(false);System.out.println("pushed\t"+text(root));
    root=root.simplify();System.out.println("simplified\t"+text(root));
    root=root.toDNF();System.out.println("dnf\t"+text(root));
    if(root instanceof LNBool&&!((LNBool)root).b){System.out.println("groups\t0");return;}
    LNDisj dnf=root instanceof LNDisj?(LNDisj)root:new LNDisj(root);
    Class<?> pem=Class.forName("tlc2.tool.liveness.Liveness$OSExprPem");
    Constructor<?> ctor=pem.getDeclaredConstructor();ctor.setAccessible(true);
    Method classify=Liveness.class.getDeclaredMethod("classifyExpr",LiveExprNode.class,pem);classify.setAccessible(true);
    Method tfs=pem.getDeclaredMethod("toTFS");tfs.setAccessible(true);
    List<LiveExprNode> keys=new ArrayList<>();List<List<Object>> groups=new ArrayList<>();
    try {
      for(int i=0;i<dnf.getCount();i++){
        Object p=ctor.newInstance();LiveExprNode branch=dnf.getBody(i).flattenSingleJunctions();
        if(branch instanceof LNConj){LNConj c=(LNConj)branch;for(int j=0;j<c.getCount();j++)classify.invoke(null,c.getBody(j),p);}
        else classify.invoke(null,branch,p);
        LiveExprNode key=(LiveExprNode)tfs.invoke(p);int at=0;
        while(at<keys.size()&&!((key==null&&keys.get(at)==null)||(key!=null&&keys.get(at)!=null&&key.equals(keys.get(at)))))at++;
        if(at==keys.size()){keys.add(key);groups.add(new ArrayList<>());}groups.get(at).add(p);
      }
    }catch(InvocationTargetException e){System.out.println("refusal\t"+e.getCause().getClass().getName()+":"+e.getCause().getMessage());if(e.getCause() instanceof util.Assert.TLCRuntimeException)System.out.println("refusal_code\t"+((util.Assert.TLCRuntimeException)e.getCause()).errorCode);return;}
    System.out.println("groups\t"+groups.size());
    for(int i=0;i<groups.size();i++){
      System.out.println("g"+i+".temporal\t"+(keys.get(i)==null?"null":text(keys.get(i))));
      List<LiveExprNode> state=new ArrayList<>(),action=new ArrayList<>();int j=0;
      for(Object p:groups.get(i)){
        String aeA=indexes(get(p,"AEAction"),action),aeS=indexes(get(p,"AEState"),state),eaA=indexes(get(p,"EAAction"),action);
        System.out.println("g"+i+".p"+(j++)+"\tAEAction="+aeA+"|AEState="+aeS+"|EAAction="+eaA);
      }
      System.out.println("g"+i+".state\t"+texts(state));System.out.println("g"+i+".action\t"+texts(action));
    }
  }
}

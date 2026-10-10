package tlc2.tool.liveness;
import java.util.*;
import tla2sany.semantic.*;
import tlc2.tool.*;
import tlc2.tool.impl.*;
import tlc2.util.*;
import tlc2.value.impl.*;
import util.*;
/** Direct pinned LNAction and Tool.eval controls; no graph/checker claim. */
public final class ActionOrderProbe {
 static ExprNode body(Tool tool,String name){
  for(OpDefNode def:tool.getSpecProcessor().getRootModule().getOpDefs())if(def.getName().toString().equals(name))return def.getBody();
  throw new IllegalArgumentException(name);
 }
 static TLCState state(int x){TLCState s=TLCState.Empty.createEmpty();s.bind(s.getVars()[0].getName(),IntValue.gen(x));return s;}
 public static void main(String[] args)throws Exception{
  java.io.PrintStream output=System.out;System.setOut(System.err);ToolIO.out=System.err;ToolIO.setUserDir(System.getProperty("user.dir"));FP64.Init(0);
  Tool tool=new FastTool("ActionOrder","ActionOrder",new SimpleFilenameToStream(),Tool.Mode.MC,new HashMap<>());System.setOut(output);
  for(String mode:new String[]{"angle","box","bad-sub","runtime"})for(int from=0;from<3;from++)for(int to=0;to<3;to++){
   String result;
   try{
    TLCState s1=state(from),s2=state(to);
    result=mode.equals("runtime")?tool.eval(body(tool,"Runtime"),Context.Empty,s1,s2,EvalControl.Clear).toString():""+new LNAction(body(tool,mode.equals("bad-sub")?"FalseBody":"Body"),Context.Empty,body(tool,mode.equals("bad-sub")?"BadSub":"Sub"),mode.equals("box")).eval(tool,s1,s2);
   }catch(EvalException e){result="error:"+e.getErrorCode();}catch(Assert.TLCRuntimeException e){result="error:"+e.errorCode;}
   System.out.println(mode+":"+from+":"+to+"\t"+result);
  }
 }
}

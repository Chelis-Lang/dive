import java.lang.reflect.*;
import java.util.*;
import tla2sany.semantic.*;
import tlc2.tool.*;
import tlc2.tool.impl.*;
import tlc2.tool.Defns;
import tlc2.util.FP64;
import util.*;
/** Actual processPeriodic snapshot lookup; no injected predicate or scheduling callbacks. */
public final class SnapshotProbe {
 static void event(String s) { System.out.println("EVENT\t"+s); }
 public static void main(String[] args) throws Exception {
  java.io.PrintStream actual=System.out;
  try {
   ToolIO.setUserDir(System.getProperty("user.dir")); ToolIO.out=System.err;
   FP64.Init(0);System.setOut(System.err);event("setup-start");
   Tool tool=new FastTool("Snapshot","Snapshot",new SimpleFilenameToStream(),Tool.Mode.MC,new HashMap<>());
   System.setOut(actual);ToolIO.out=System.out;event("setup-complete");
   SpecProcessor processor=tool.getSpecProcessor();
   String name=tool.getModelConfig().getPeriodic();event("configured-name:"+name);
   Field field=SpecProcessor.class.getDeclaredField("snapshot");field.setAccessible(true);
   Object entry=((Defns)field.get(processor)).get(name);
   event("snapshot-class:"+(entry==null?"null":entry.getClass().getName()));
   ExprNode body=processor.getPeriodic();event("body-present:"+(body!=null));
   if(entry instanceof OpDefNode) {
    OpDefNode def=(OpDefNode)entry;
    event("definition-name:"+def.getName());event("definition-arity:"+def.getArity());
    event("body-identical:"+(body==def.getBody()));
   }
   if(body!=null){event("body-class:"+body.getClass().getName());event("body-kind:"+body.getKind());event("body-level:"+body.getLevel());event("body-location:"+body.getLocation());
    event("evaluation-start");Object value=tool.eval(body);event("value-class:"+value.getClass().getName());event("value:"+value);}
  } catch(Throwable t) {
   System.setOut(actual);event("exception-class:"+t.getClass().getName());
   int code=t instanceof EvalException?((EvalException)t).getErrorCode():t instanceof Assert.TLCRuntimeException?((Assert.TLCRuntimeException)t).errorCode:-1;
   event("exception-code:"+code);System.out.println("MESSAGE-BEGIN");System.out.println(t.getMessage());System.out.println("MESSAGE-END");t.printStackTrace(System.err);System.exit(2);
  }
 }
}

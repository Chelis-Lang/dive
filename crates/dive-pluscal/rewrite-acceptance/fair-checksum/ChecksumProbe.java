// Development-only adapter to pinned public rewrite and checksum contracts.
package pcal;
import java.nio.file.*;
import java.nio.charset.StandardCharsets;
import java.lang.reflect.Method;
import java.util.*;
public final class ChecksumProbe {
 public static void main(String[] args) throws Exception {
  PcalParams.resetParams();
  List<String> input=Files.readAllLines(Path.of(args[1]),StandardCharsets.UTF_8);
  if(args[0].equals("validate")) {
   Method m=Validator.class.getDeclaredMethod("validate",List.class); m.setAccessible(true);
   Object result=m.invoke(null,input); System.out.println("VALIDATION\t"+result);return;
  }
  if(args[0].equals("crc")) {System.out.println("CHECKSUM\t"+Validator.checksum(new Vector<>(input)));return;}
  final boolean cancel=args[0].equals("cancel");
  final ValidationCallBack.Generate generation=args[0].equals("ignore")?ValidationCallBack.Generate.IGNORE:args[0].equals("generate")?ValidationCallBack.Generate.DO_IT:ValidationCallBack.Generate.NOT_NOW;
  List<String> output=trans.performTranslation(input,new ValidationCallBack(){public boolean shouldCancel(){return cancel;}public Generate shouldGenerate(){return generation;}});
  if(output==null){System.out.println("OUTCOME\trejected");return;}
  Files.writeString(Path.of(args[2]),String.join("\n",output)+"\n",StandardCharsets.UTF_8);
  System.out.println("OUTCOME\ttranslated");
 }
}

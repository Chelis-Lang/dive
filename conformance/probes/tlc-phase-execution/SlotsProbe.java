package tlc2.tool.liveness;

import java.lang.reflect.Field;
import java.util.*;
import tla2sany.semantic.OpDeclNode;
import tla2sany.semantic.SymbolNode;
import tlc2.tool.TLCState;
import tlc2.tool.StateVec;
import tlc2.util.SetOfStates;
import tlc2.value.IValue;
import tlc2.value.impl.*;
import util.UniqueString;

/** Diagnostic forced-fingerprint controls. Equality invokes actual TLC values.
 * No claim of model execution or fingerprint derivation is made here. */
public final class SlotsProbe {
  static final class State extends TLCState {
    final long fingerprint;
    final String label;
    final Value value;
    State(long f,String l,Value v){fingerprint=f;label=l;value=v;}
    public long fingerPrint(){return fingerprint;}
    public boolean equals(Object o){return o instanceof State && value.equals(((State)o).value);}
    public int hashCode(){return value.hashCode();}
    public TLCState bind(UniqueString n,IValue v){throw new UnsupportedOperationException();}
    public TLCState bind(SymbolNode n,IValue v){throw new UnsupportedOperationException();}
    public TLCState unbind(UniqueString n){throw new UnsupportedOperationException();}
    public IValue lookup(UniqueString n){return value;}
    public boolean containsKey(UniqueString n){return true;}
    public TLCState copy(){return this;}
    public TLCState deepCopy(){return this;}
    public StateVec addToVec(StateVec v){v.addElement(this);return v;}
    public void deepNormalize(){}
    public boolean allAssigned(){return true;}
    public Set<OpDeclNode> getUnassigned(){return Collections.emptySet();}
    public TLCState createEmpty(){throw new UnsupportedOperationException();}
    public String toString(){return label;}
    public String toString(TLCState s){return label;}
    public String toString(UniqueString[] v,TLCState s){return label;}
  }
  static Value value(String s){
    switch(s.charAt(0)) {
      case 'i':return IntValue.gen(Integer.parseInt(s.substring(1)));
      case 'b':return new BoolValue(Boolean.parseBoolean(s.substring(1)));
      case 's':return new StringValue(s.substring(1));
      default:throw new IllegalArgumentException(s);
    }
  }
  static TLCState[] slots(SetOfStates set)throws Exception {
    Field f=SetOfStates.class.getDeclaredField("states");f.setAccessible(true);
    return (TLCState[])f.get(set);
  }
  static String contents(SetOfStates set)throws Exception {
    List<String> out=new ArrayList<>();TLCState[] slots=slots(set);
    for(int i=0;i<slots.length;i++)if(slots[i]!=null)out.add(i+":"+slots[i]);
    return String.join(";",out);
  }
  public static void main(String[] args)throws Exception {
    SetOfStates set=new SetOfStates(Integer.parseInt(args[0]));
    System.out.println("initial.capacity\t"+set.capacity());
    int step=0;
    if(!args[1].isEmpty())for(String record:args[1].split(";")) {
      String[] fields=record.split(":",3);
      State state=new State(Long.parseUnsignedLong(fields[0]),fields[1],value(fields[2]));
      boolean duplicate=set.put(state);
      String p="p"+(step++);
      System.out.println(p+".duplicate\t"+duplicate);
      System.out.println(p+".count\t"+set.size());
      System.out.println(p+".capacity\t"+set.capacity());
      System.out.println(p+".slots\t"+contents(set));
    }
    List<String> iteration=new ArrayList<>();for(int i=0;i<set.size();i++)iteration.add(set.next().toString());
    System.out.println("iteration\t"+String.join(";",iteration));
    set.resetNext();List<String> repeat=new ArrayList<>();for(int i=0;i<set.size();i++)repeat.add(set.next().toString());
    System.out.println("reset.iteration\t"+String.join(";",repeat));
  }
}

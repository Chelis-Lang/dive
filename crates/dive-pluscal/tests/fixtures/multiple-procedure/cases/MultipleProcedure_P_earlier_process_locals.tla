---- MODULE MultipleProcedure_P_earlier_process_locals ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=x+a; R:return;
end procedure;
process One = 1
variable y=1,q=y+1;
begin
L:call First(1); E:skip;
end process;
process Two \in {2}
variable z=self,t=z+1;
begin
M:call First(2); N:skip;
end process;
end algorithm; *)
====

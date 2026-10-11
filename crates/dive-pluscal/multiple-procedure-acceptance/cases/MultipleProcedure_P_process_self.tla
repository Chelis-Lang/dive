---- MODULE MultipleProcedure_P_process_self ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=x+a; R:return;
end procedure;
process One \in {1}

begin
L:call First(self); E:skip;
end process;
process Two \in {2}

begin
M:call First(self); N:skip;
end process;
end algorithm; *)
====

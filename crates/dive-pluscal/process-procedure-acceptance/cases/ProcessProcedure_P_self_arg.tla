---- MODULE ProcessProcedure_P_self_arg ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=a; R:return;
end procedure;

process Worker \in {1,2}

begin
L:call First(self); E:skip;
end process;
end algorithm; *)
====

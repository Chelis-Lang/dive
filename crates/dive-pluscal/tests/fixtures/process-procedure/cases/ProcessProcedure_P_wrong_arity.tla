---- MODULE ProcessProcedure_P_wrong_arity ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=a; R:return;
end procedure;

process Worker = 1

begin
L:call First(); E:skip;
end process;
end algorithm; *)
====

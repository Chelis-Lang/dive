---- MODULE ProcessProcedure_P_missing_return ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=a;
end procedure;

process Worker = 1

begin
L:call First(2); E:skip;
end process;
end algorithm; *)
====

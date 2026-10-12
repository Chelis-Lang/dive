---- MODULE ProcessProcedure_P_no_params_tail ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First()

begin
F:call First(); return;
end procedure;

process Worker = 1

begin
L:call First(); E:skip;
end process;
end algorithm; *)
====

---- MODULE FairProcedure_P_invalid_no_param_tail ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First()

begin
F:call First(); return;
end procedure;
fair process One = 1

begin
L:call First(); E:skip;
end process;
end algorithm; *)
====

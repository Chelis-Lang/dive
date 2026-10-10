---- MODULE FairProcedure_P_constants_both_fair_option_nof ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=x+a; R:return;
end procedure;
fair process One = 1

begin
L:call First(1); E:skip;
end process;
fair + process Two = 2

begin
M:call First(2); N:skip;
end process;
end algorithm; *)
====

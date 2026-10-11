---- MODULE FairProcedure_P_member_algorithm_nof ----
EXTENDS Integers, Sequences, TLC
(* --fair algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=x+a; R:return;
end procedure;
process One \in {1,2}

begin
L:call First(1); E:skip;
end process;
end algorithm; *)
====

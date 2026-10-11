---- MODULE FairProcedureModifier_P_mixed_reach_nested_modifiers ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F: - call Second(a); R: + return;
end procedure;
procedure Second(b=0)

begin
S: + x:=x+b; T: - return;
end procedure;
fair process One = 1

begin
L: + call First(1); E:skip;
end process;
fair + process Two \in {2,3}

begin
M: - call Second(2); N:skip;
end process;
end algorithm; *)
====

---- MODULE FairProcedure_P_fair_domain_multi_line ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=x+a; R:return;
end procedure;
fair process One \in {1,
2}

begin
L:call First(1); E:skip;
end process;
end algorithm; *)
====

---- MODULE FairProcedure_P_invalid_constant_self ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=x+a; R:return;
end procedure;
fair process One = 1

begin
L:call First(self); E:skip;
end process;
end algorithm; *)
====

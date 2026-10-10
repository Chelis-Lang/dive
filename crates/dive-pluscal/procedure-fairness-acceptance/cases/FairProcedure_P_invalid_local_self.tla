---- MODULE FairProcedure_P_invalid_local_self ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)
variable u=self;
begin
F:x:=x+u; R:return;
end procedure;
fair process One = 1

begin
L:call First(1); E:skip;
end process;
end algorithm; *)
====

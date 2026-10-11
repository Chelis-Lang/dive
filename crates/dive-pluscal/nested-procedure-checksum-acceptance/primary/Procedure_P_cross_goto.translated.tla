---- MODULE Procedure_P_cross_goto ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)

begin
P: goto E;
end procedure;
begin
L: call Put(2); E: skip;
end algorithm; *)
====

---- MODULE Procedure_P_reserved_pc ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(pc=0)

begin
P: return;
end procedure;
begin
L: call Put(2); E: skip;
end algorithm; *)
====

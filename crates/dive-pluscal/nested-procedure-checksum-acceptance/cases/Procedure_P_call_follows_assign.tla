---- MODULE Procedure_P_call_follows_assign ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)

begin
P: x:=a; return;
end procedure;
begin
L: x:=1; call Put(x+1); E: skip;
end algorithm; *)
====

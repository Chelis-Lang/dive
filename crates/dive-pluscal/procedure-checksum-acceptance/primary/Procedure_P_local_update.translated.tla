---- MODULE Procedure_P_local_update ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)
variable b=1;
begin
P: b:=a+1; x:=b; return;
end procedure;
begin
L: call Put(2); E: skip;
end algorithm; *)
====

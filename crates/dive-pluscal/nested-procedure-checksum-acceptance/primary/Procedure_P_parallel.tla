---- MODULE Procedure_P_parallel ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0,b=1)

begin
P: a:=b || b:=a; R: x:=a; return;
end procedure;
begin
L: call Put(2,3); E: skip;
end algorithm; *)
====

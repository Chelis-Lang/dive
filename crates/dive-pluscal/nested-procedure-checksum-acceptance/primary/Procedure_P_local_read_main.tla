---- MODULE Procedure_P_local_read_main ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)
variable b=1;
begin
P: x:=a; return;
end procedure;
begin
L: call Put(2); E: x:=b;
end algorithm; *)
====

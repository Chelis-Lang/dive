---- MODULE Procedure_P_loop ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)

begin
P: while a<3 do a:=a+1; end while; R: x:=a; return;
end procedure;
begin
L: call Put(0); E: skip;
end algorithm; *)
====

---- MODULE Procedure_P_call_to_done ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)

begin
P: x:=a; R: return;
end procedure;
begin
L: call Put(2);
end algorithm; *)
====

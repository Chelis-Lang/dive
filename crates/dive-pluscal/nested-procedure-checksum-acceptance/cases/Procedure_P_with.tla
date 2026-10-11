---- MODULE Procedure_P_with ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)

begin
P: with b=a+1 do x:=b; end with; return;
end procedure;
begin
L: call Put(2); E: skip;
end algorithm; *)
====

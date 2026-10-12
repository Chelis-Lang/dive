---- MODULE Procedure_P_auto ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)

begin
x:=a; return;
end procedure;
begin
call Put(2); skip;
end algorithm; *)
====

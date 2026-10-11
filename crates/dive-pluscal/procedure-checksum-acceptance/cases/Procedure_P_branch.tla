---- MODULE Procedure_P_branch ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)

begin
P: if a>0 then x:=a; else x:=1; end if; return;
end procedure;
begin
L: call Put(2); E: skip;
end algorithm; *)
====

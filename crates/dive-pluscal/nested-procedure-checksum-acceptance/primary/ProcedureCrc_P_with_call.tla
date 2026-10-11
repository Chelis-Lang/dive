---- MODULE ProcedureCrc_P_with_call ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)

begin
P:x:=a; R:return;
end procedure;
begin
L:with q=2 do call Put(q); goto E; end with; E:skip;
end algorithm; *)
====

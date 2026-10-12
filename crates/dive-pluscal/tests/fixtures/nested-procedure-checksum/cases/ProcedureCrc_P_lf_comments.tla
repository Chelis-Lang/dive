---- MODULE ProcedureCrc_P_lf_comments ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)
variable t=(* initial *) a+1;
begin
P:x:=(* actual *) t; R:return;
end procedure;
begin
L:call Put(2); E:skip;
end algorithm; *)
====

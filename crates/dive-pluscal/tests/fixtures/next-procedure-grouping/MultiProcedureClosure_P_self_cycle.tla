---- MODULE MultiProcedureClosure_P_self_cycle ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)
begin
F:call First(a); R:return;
end procedure;
procedure Second(b=0)
begin
G:x:=b; H:return;
end procedure;
process One = 1
begin
L:call First(1); E:skip;
end process;
process Two = 2
begin
S:call Second(2); T:skip;
end process;
end algorithm; *)
====

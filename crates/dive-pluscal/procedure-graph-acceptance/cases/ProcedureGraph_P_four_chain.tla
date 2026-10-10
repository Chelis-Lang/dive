---- MODULE ProcedureGraph_P_four_chain ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:call Second(a); R:return;
end procedure;
procedure Second(b=1)

begin
S:call Third(b); T:return;
end procedure;
procedure Third(c=0)

begin
U:call Fourth(c); V:return;
end procedure;
procedure Fourth(d=0)

begin
W:x:=d; Q:return;
end procedure;
begin
L: call First(2); E: skip;
end algorithm; *)
====

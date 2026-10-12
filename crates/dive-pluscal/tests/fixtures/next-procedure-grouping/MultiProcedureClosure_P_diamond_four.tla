---- MODULE MultiProcedureClosure_P_diamond_four ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)
begin
F:call Second(a); R:call Third(a); O:return;
end procedure;
procedure Second(b=0)
begin
G:call Fourth(b); H:return;
end procedure;
procedure Third(c=0)
begin
I:call Fourth(c); J:return;
end procedure;
procedure Fourth(d=0)
begin
K:x:=d; N:return;
end procedure;
process One = 1
begin
L:call First(1); E:skip;
end process;
process Two = 2
begin
S:call Fourth(2); T:skip;
end process;
end algorithm; *)
====

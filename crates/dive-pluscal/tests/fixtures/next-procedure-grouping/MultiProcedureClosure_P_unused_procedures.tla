---- MODULE MultiProcedureClosure_P_unused_procedures ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)
begin
F:call Second(a); R:return;
end procedure;
procedure Second(b=0)
begin
G:call Third(b); H:return;
end procedure;
procedure Third(c=0)
begin
I:x:=c; J:return;
end procedure;
process One = 1
begin
L:skip; E:skip;
end process;
process Two = 2
begin
S:skip; T:skip;
end process;
end algorithm; *)
====

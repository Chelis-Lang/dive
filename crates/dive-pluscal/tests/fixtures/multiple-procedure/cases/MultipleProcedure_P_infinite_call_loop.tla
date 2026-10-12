---- MODULE MultipleProcedure_P_infinite_call_loop ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=x+a; R:return;
end procedure;
process One = 1

begin
L:while TRUE do call First(1); end while;
end process;
process Two = 2

begin
M:call First(2); N:skip;
end process;
end algorithm; *)
====
